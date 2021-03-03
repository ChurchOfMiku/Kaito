use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{prelude::*, Error as LuaError, Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;
use thiserror::Error;

use super::super::state::LuaAsyncCallback;
use crate::{
    bot::{db::User as DbUser, Bot, ROLES},
    services::{
        Channel, ChannelId, Message, Server, ServerId, Service, ServiceKind, ServiceUserId,
        Services, User,
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

                Ok(BotUser {
                    name: service_user.name().to_string(),
                    user,
                    restricted,
                })
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
                bot.db().set_role_for_user(user.user.uid, &role),
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
            let restrictor_msg = restrictor.borrow::<BotMessage>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                bot.db()
                    .restrict_user(user.user.uid, restrictor_msg.user.uid),
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
            bot.db().unrestrict_user(user.user.uid),
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
                        SettingContext::Server(msg.server_id())
                    } else {
                        SettingContext::Channel(msg.channel_id())
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
pub struct BotMessage {
    bot: Arc<Bot>,
    user_id: ServiceUserId,
    channel_id: ChannelId,
    server_id: ServerId,
    content: String,
    user: DbUser,
    service: ServiceKind,
}

impl BotMessage {
    pub async fn from_msg(
        bot: Arc<Bot>,
        msg: &Arc<dyn Message<impl Service>>,
    ) -> Result<BotMessage> {
        let user = bot
            .db()
            .get_user_from_service_user_id(msg.author().id())
            .await?;
        let channel = msg.channel().await?;

        Ok(BotMessage {
            bot,
            user_id: msg.author().id(),
            channel_id: channel.id(),
            server_id: channel.server().await?.id(),
            content: msg.content().to_string(),
            user,
            service: msg.service().kind(),
        })
    }

    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    pub fn server_id(&self) -> ServerId {
        self.server_id
    }
}

impl UserData for BotMessage {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("reply", |_state, msg, content: String| {
            let ctx = msg.bot.get_ctx();
            let channel_id = msg.channel_id;

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
                "user_uid" => Ok(mlua::Value::Number(msg.user.uid as f64)),
                "content" => Ok(mlua::Value::String(
                    state.create_string(msg.content.as_bytes())?,
                )),
                "user_id" => Ok(mlua::Value::String(
                    state.create_string(msg.user_id.to_short_str().as_bytes())?,
                )),
                "role" => Ok(mlua::Value::String(
                    state.create_string(msg.user.role.as_bytes())?,
                )),
                "service" => Ok(mlua::Value::String(
                    state.create_string(Services::id_from_kind(msg.service).as_bytes())?,
                )),
                _ => Ok(mlua::Value::Nil),
            }
        });
    }
}

#[derive(Clone)]
pub struct BotUser {
    name: String,
    user: DbUser,
    restricted: bool,
}

impl UserData for BotUser {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_meta_method(
            MetaMethod::Index,
            |state, user, index: String| match index.as_str() {
                "uid" => Ok(mlua::Value::Number(user.user.uid as f64)),
                "name" => Ok(mlua::Value::String(
                    state.create_string(user.name.as_bytes())?,
                )),
                "role" => Ok(mlua::Value::String(
                    state.create_string(user.user.role.as_bytes())?,
                )),
                "restricted" => Ok(mlua::Value::Boolean(user.restricted)),
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
