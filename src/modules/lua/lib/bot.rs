use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{prelude::*, Error as LuaError, Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;
use thiserror::Error;

use super::super::state::LuaAsyncCallback;
use crate::{
    bot::{
        db::{User as DbUser, UserId},
        Bot, ROLES,
    },
    services::{
        Channel, ChannelId, Message, Server, ServerId, Service, ServiceKind, Services, User,
    },
    settings::SettingContext,
};

pub fn lib_bot(state: &Lua, bot: &Arc<Bot>, sender: Sender<LuaAsyncCallback>) -> Result<()> {
    let bot_tbl = state.create_table()?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let bot_restart_sandbox_fn = state.create_function(move |state, (): ()| {
        let ctx = bot2.get_ctx();

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            async move {
                if let Err(err) = ctx.modules().lua.module().restart_sandbox().await {
                    println!("error restarting sandbox: {}", err.to_string());
                }
            },
            |_state, _data: (), _res: ()| { Ok(()) }
        );

        Ok(fut)
    })?;
    bot_tbl.set("restart_sandbox", bot_restart_sandbox_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let find_user_fn = state.create_function(move |state, (service, user): (String, String)| {
        let ctx = bot2.get_ctx();

        let service = match ServiceKind::from_str(&service) {
            Some(kind) => kind,
            None => {
                return Err(LuaError::ExternalError(Arc::new(BotError::UnknownService(
                    service,
                ))))
            }
        };

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            async move {
                match ctx.services().find_user(service, &user).await {
                    Ok(service_user) => match ctx
                        .bot()
                        .db()
                        .get_user_from_service_user_id(service_user.id())
                        .await
                    {
                        Ok(user) => ctx
                            .bot()
                            .db()
                            .is_restricted(user.uid)
                            .await
                            .map(|restricted| (service_user, user, restricted)),
                        Err(err) => Err(err),
                    },
                    Err(err) => Err(err),
                }
            },
            |_state, _data: (), res: Result<(Arc<dyn User<impl Service>>, DbUser, bool)>| {
                let (service_user, user, restricted): (Arc<dyn User<_>>, _, _) = res?;

                Ok(BotUser(
                    Arc::new(BotUserInner {
                        name: service_user.name().to_string(),
                        restricted,
                    }),
                    Arc::new(user),
                ))
            }
        );

        Ok(fut)
    })?;
    bot_tbl.set("find_user", find_user_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let set_role_fn =
        state.create_function(move |state, (user, role): (LuaAnyUserData, String)| {
            let bot = bot2.clone();

            let user = user.borrow::<BotUser>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                bot.db().set_role_for_user(user.uid(), &role),
                |_state, _data: (), res: Result<()>| { res }
            );

            Ok(fut)
        })?;
    bot_tbl.set("set_role", set_role_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let restrict_user_fn = state.create_function(
        move |state, (user, restrictor): (LuaAnyUserData, LuaAnyUserData)| {
            let bot = bot2.clone();

            let user = user.borrow::<BotUser>()?.clone();
            let restrictor = restrictor.borrow::<BotUser>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                bot.db().restrict_user(user.uid(), restrictor.uid()),
                |_state, _data: (), res: Result<()>| { res }
            );

            Ok(fut)
        },
    )?;
    bot_tbl.set("restrict_user", restrict_user_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let unrestrict_user_fn = state.create_function(move |state, (user,): (LuaAnyUserData,)| {
        let bot = bot2.clone();

        let user = user.borrow::<BotUser>()?.clone();

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            bot.db().unrestrict_user(user.uid()),
            |_state, _data: (), res: Result<()>| { res }
        );

        Ok(fut)
    })?;
    bot_tbl.set("unrestrict_user", unrestrict_user_fn)?;

    let bot2 = bot.clone();
    let list_settings_fn = state.create_function(move |state, (module,): (String,)| {
        let bot = bot2.clone();

        let module_settings = match bot.get_ctx().modules().get_settings(&module) {
            Some(settings) => settings,
            None => return Ok(LuaMultiValue::new()),
        };

        let tbl = state.create_table()?;

        for (idx, info) in module_settings.enumerate().into_iter().enumerate() {
            let info_tbl = state.create_table()?;

            info_tbl.set("name", info.name)?;
            info_tbl.set("help", info.help)?;

            tbl.raw_insert((idx + 1) as i64, info_tbl)?;
        }

        Ok(LuaMultiValue::from_vec(vec![LuaValue::Table(tbl)]))
    })?;
    bot_tbl.set("list_settings", list_settings_fn)?;

    let bot2 = bot.clone();
    let set_setting_fn = state.create_function(
        move |state,
              (msg, server, module, setting, value): (
            LuaAnyUserData,
            bool,
            String,
            String,
            String,
        )| {
            let bot = bot2.clone();

            let module_settings = match bot.get_ctx().modules().get_settings(&module) {
                Some(settings) => settings,
                None => {
                    return Ok(LuaMultiValue::from_vec(vec![
                        "unknown module".to_lua(state)?
                    ]))
                }
            };

            let msg = msg.borrow::<BotMessage>()?.clone();

            let fut = create_lua_future!(
                state,
                sender,
                (),
                module_settings.set_setting(
                    if server {
                        SettingContext::Server(msg.server().id())
                    } else {
                        SettingContext::Channel(msg.channel().id())
                    },
                    &setting,
                    &value,
                ),
                |_state, _data: (), res: Result<()>| { res }
            );

            Ok(LuaMultiValue::from_vec(vec![
                LuaValue::Nil,
                LuaValue::Table(fut),
            ]))
        },
    )?;
    bot_tbl.set("set_setting", set_setting_fn)?;

    bot_tbl.set("ROLES", ROLES)?;

    state.globals().set("bot", bot_tbl)?;

    Ok(())
}

#[derive(Clone)]
pub struct BotMessage(Arc<BotMessageInner>);

pub struct BotMessageInner {
    bot: Arc<Bot>,
    author: BotUser,
    channel: BotChannel,
    content: String,
    service: ServiceKind,
    server: BotServer,
}

impl BotMessage {
    pub async fn from_msg(
        bot: Arc<Bot>,
        msg: &Arc<dyn Message<impl Service>>,
    ) -> Result<BotMessage> {
        let service_user = msg.author().clone() as Arc<dyn User<_>>;
        let author = BotUser::from_user(bot.clone(), &service_user).await?;
        let service_channel = msg.channel().await? as Arc<dyn Channel<_>>;
        let channel = BotChannel::from_channel(&service_channel).await?;
        let service_server = service_channel.server().await? as Arc<dyn Server<_>>;
        let server = BotServer::from_server(&service_server).await?;

        Ok(BotMessage(Arc::new(BotMessageInner {
            bot,
            author,
            channel,
            content: msg.content().to_string(),
            service: msg.service().kind(),
            server,
        })))
    }

    pub fn author(&self) -> &BotUser {
        &self.0.author
    }

    pub fn channel(&self) -> &BotChannel {
        &self.0.channel
    }

    pub fn server(&self) -> &BotServer {
        &self.0.server
    }
}

impl UserData for BotMessage {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("reply", |_state, msg, content: String| {
            let ctx = msg.0.bot.get_ctx();
            let channel_id = msg.0.channel.id();

            tokio::spawn(async move {
                ctx.services()
                    .clone()
                    .send_message(channel_id, content)
                    .await
                    .ok();
            });

            Ok(())
        });

        methods.add_meta_method(MetaMethod::Index, |state, msg, index: String| {
            match index.as_str() {
                "author" => Ok(mlua::Value::UserData(
                    state.create_userdata(msg.0.author.clone())?,
                )),
                "content" => Ok(mlua::Value::String(
                    state.create_string(msg.0.content.as_bytes())?,
                )),
                "service" => Ok(mlua::Value::String(
                    state.create_string(Services::id_from_kind(msg.0.service).as_bytes())?,
                )),
                _ => Ok(mlua::Value::Nil),
            }
        });
    }
}

#[derive(Clone)]
pub struct BotUser(Arc<BotUserInner>, Arc<DbUser>);

impl BotUser {
    pub async fn from_user(
        bot: Arc<Bot>,
        service_user: &Arc<dyn User<impl Service>>,
    ) -> Result<BotUser> {
        let user = bot
            .db()
            .get_user_from_service_user_id(service_user.id())
            .await?;
        let restricted = bot.db().is_restricted(user.uid).await?;

        Ok(BotUser(
            Arc::new(BotUserInner {
                name: service_user.name().to_string(),
                restricted,
            }),
            Arc::new(user),
        ))
    }

    pub fn uid(&self) -> UserId {
        self.1.uid
    }
}

pub struct BotUserInner {
    name: String,
    restricted: bool,
}

impl UserData for BotUser {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_meta_method(
            MetaMethod::Index,
            |state, user, index: String| match index.as_str() {
                "uid" => Ok(mlua::Value::Number(user.1.uid as f64)),
                "name" => Ok(mlua::Value::String(
                    state.create_string(user.0.name.as_bytes())?,
                )),
                "role" => Ok(mlua::Value::String(
                    state.create_string(user.1.role.as_bytes())?,
                )),
                "restricted" => Ok(mlua::Value::Boolean(user.0.restricted)),
                _ => Ok(mlua::Value::Nil),
            },
        );
    }
}

#[derive(Clone)]
pub struct BotChannel(Arc<BotChannelInner>);

pub struct BotChannelInner {
    id: ChannelId,
}

impl BotChannel {
    pub async fn from_channel(channel: &Arc<dyn Channel<impl Service>>) -> Result<BotChannel> {
        Ok(BotChannel(Arc::new(BotChannelInner { id: channel.id() })))
    }

    pub fn id(&self) -> ChannelId {
        self.0.id
    }
}

impl UserData for BotChannel {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_meta_method(
            MetaMethod::Index,
            |state, user, index: String| match index.as_str() {
                "id" => Ok(mlua::Value::String(
                    state.create_string(&user.0.id.to_short_str())?,
                )),
                _ => Ok(mlua::Value::Nil),
            },
        );
    }
}

#[derive(Clone)]
pub struct BotServer(Arc<BotServerInner>);

pub struct BotServerInner {
    id: ServerId,
}

impl BotServer {
    pub async fn from_server(server: &Arc<dyn Server<impl Service>>) -> Result<BotServer> {
        Ok(BotServer(Arc::new(BotServerInner { id: server.id() })))
    }

    pub fn id(&self) -> ServerId {
        self.0.id
    }
}

impl UserData for BotServer {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_meta_method(
            MetaMethod::Index,
            |state, user, index: String| match index.as_str() {
                "id" => Ok(mlua::Value::String(
                    state.create_string(&user.0.id.to_short_str())?,
                )),
                _ => Ok(mlua::Value::Nil),
            },
        );
    }
}

#[derive(Debug, Error)]
pub enum BotError {
    #[error("unknown service \"{}\"", _0)]
    UnknownService(String),
}
