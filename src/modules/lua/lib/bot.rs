use anyhow::Result;
use async_mutex::Mutex;
use crossbeam::channel::{Sender, TryRecvError};
use mlua::{prelude::*, Error as LuaError, Lua, MetaMethod, Table, UserData, UserDataMethods};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use super::super::state::{LuaAsyncCallback, LuaState, SandboxMsg, SandboxTerminationReason};
use crate::{
    bot::{
        db::{User as DbUser, UserId},
        Bot, ROLES,
    },
    services::{
        Channel, ChannelId, Message, MessageId, Server, ServerId, Service, ServiceFeatures,
        ServiceKind, ServiceUserId, Services, User,
    },
    settings::SettingContext,
    utils::escape_untrusted_text,
};

pub fn lib_bot(
    state: &Lua,
    bot: &Arc<Bot>,
    sender: Sender<LuaAsyncCallback>,
    sandbox_state: Arc<Mutex<LuaState>>,
) -> Result<()> {
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
    let get_user_fn = state.create_function(move |state, user_id: i64| {
        let bot = bot2.clone();

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            async move {
                let user = bot.db().get_user_from_uid(user_id).await;
                let ctx = bot.get_ctx();

                match user {
                    Ok(user) => {
                        Ok((futures::join!(
                            ctx.services().user(user.service_user_id()),
                            bot.db().is_restricted(user_id),
                        ), user))
                    },
                    Err(err) => Err(err)
                }
            },
            |_state,
             _data: (),
             res: Result<((Result<Arc<dyn User<impl Service>>>, Result<bool>), DbUser)>| {
                let (res, user) = res?;
                let (service_user, restricted): (Arc<dyn User<_>>, _) = (res.0?, res.1?);

                Ok(BotUser(
                    Arc::new(BotUserInner {
                        name: service_user.name().to_string(),
                        nick: service_user.nick().to_string(),
                        id: service_user.id(),
                        restricted,
                    }),
                    Arc::new(user),
                ))
            }
        );

        Ok(fut)
    })?;
    bot_tbl.set("get_user", get_user_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let find_user_fn =
        state.create_function(move |state, (channel, user): (LuaAnyUserData, String)| {
            let ctx = bot2.get_ctx();

            let channel = channel.borrow::<BotChannel>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                async move {
                    match ctx.services().find_user(channel.id(), &user).await {
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
                            nick: service_user.nick().to_string(),
                            id: service_user.id(),
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
    let sender2 = sender.clone();
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
                sender2,
                (),
                module_settings.set_setting(
                    if server {
                        SettingContext::Server(msg.channel().server().id())
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

    let run_sandboxed_lua_fn = state.create_function(
        move |state,
              (user, code, env): (
            LuaAnyUserData,
            String,
            Table
        )| {
            let sandbox_state = sandbox_state.clone();

            let _user = user.borrow::<BotUser>()?.clone();

            let env_encoded: String = serde_json::to_string(&LuaValue::Table(env))
                .map_err(|err| LuaError::ExternalError(Arc::new(err)))?;

            let fut = create_lua_future!(
                state,
                sender,
                (),
                async move {
                    let lua_state = sandbox_state.lock_arc().await;

                    let (_sandbox_state, recv) = match lua_state.run_sandboxed(&code, Some(env_encoded)) {
                        Ok(recv) => recv,
                        Err(err) => {
                            return Err(anyhow::anyhow!(err.to_string()));
                        }
                    };

                    drop(lua_state);

                    let mut out_str = String::new();
                    let start = Instant::now();

                    loop {
                        if start.elapsed() > Duration::from_secs(2) {
                            break;
                        }

                        match recv.try_recv() {
                            Ok(out) => match out {
                                SandboxMsg::Out(o) => {
                                    out_str.push_str(&o);
                                }
                                SandboxMsg::Error(err) => {
                                    return Err(anyhow::anyhow!(err));
                                }
                                SandboxMsg::Terminated(reason) => {
                                    match reason {
                                        SandboxTerminationReason::Done => {
                                            break
                                        },
                                        SandboxTerminationReason::ExecutionQuota => {
                                            return Err(anyhow::anyhow!("Execution quota exceeded, terminated execution"));
                                        }
                                        SandboxTerminationReason::TimeLimit => {
                                            return Err(anyhow::anyhow!("Execution time limit reached, terminated execution"));
                                        }
                                    }
                                }
                            },
                            Err(TryRecvError::Empty) => {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                            Err(TryRecvError::Disconnected) => break,
                        }
                    }

                    Ok(out_str)
                },
                |state, _data: (), res: Result<String>| {
                    match res {
                        Ok(res) => Ok(LuaMultiValue::from_vec(vec![LuaValue::Nil, LuaValue::String(state.create_string(&res)?)])),
                        Err(err) => Ok(LuaMultiValue::from_vec(vec![LuaValue::String(state.create_string(&err.to_string())?)])),
                    }
                }
            );

            Ok(fut)
        },
    )?;
    bot_tbl.set("run_sandboxed_lua", run_sandboxed_lua_fn)?;

    bot_tbl.set("ROLES", ROLES)?;

    let features_tbl = state.create_table()?;

    features_tbl.set("Edit", ServiceFeatures::EDIT.bits())?;
    features_tbl.set("Embed", ServiceFeatures::EMBED.bits())?;
    features_tbl.set("React", ServiceFeatures::REACT.bits())?;
    features_tbl.set("Voice", ServiceFeatures::VOICE.bits())?;
    features_tbl.set("Markdown", ServiceFeatures::MARKDOWN.bits())?;

    bot_tbl.set("FEATURES", features_tbl)?;

    state.globals().set("bot", bot_tbl)?;

    Ok(())
}

#[derive(Clone)]
pub struct BotMessage(Arc<BotMessageInner>);

pub struct BotMessageInner {
    bot: Arc<Bot>,
    sender: Sender<LuaAsyncCallback>,
    id: MessageId,
    author: BotUser,
    channel: BotChannel,
    content: String,
    service: ServiceKind,
}

impl BotMessage {
    pub async fn from_msg(
        bot: Arc<Bot>,
        sender: Sender<LuaAsyncCallback>,
        msg: &Arc<dyn Message<impl Service>>,
    ) -> Result<BotMessage> {
        let service_user = msg.author().clone() as Arc<dyn User<_>>;
        let author = BotUser::from_user(bot.clone(), &service_user).await?;
        let service_channel = msg.channel().await? as Arc<dyn Channel<_>>;
        let channel =
            BotChannel::from_channel(bot.clone(), sender.clone(), &service_channel).await?;

        Ok(BotMessage(Arc::new(BotMessageInner {
            bot,
            sender,
            id: msg.id(),
            author,
            channel,
            content: msg.content().to_string(),
            service: msg.service().kind(),
        })))
    }

    pub fn author(&self) -> &BotUser {
        &self.0.author
    }

    pub fn channel(&self) -> &BotChannel {
        &self.0.channel
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

        methods.add_method("react", |state, msg, reaction: String| {
            let ctx = msg.0.bot.get_ctx();
            let channel_id = msg.channel().id();
            let msg_id = msg.0.id;

            let fut = create_lua_future!(
                state,
                msg.0.sender,
                (),
                ctx.services().react(channel_id, msg_id, reaction),
                |_state, _data: (), res: Result<()>| { Ok(res?) }
            );

            Ok(fut)
        });

        methods.add_method("edit", |state, msg, content: String| {
            let ctx = msg.0.bot.get_ctx();
            let channel_id = msg.channel().id();
            let msg_id = msg.0.id;

            let fut = create_lua_future!(
                state,
                msg.0.sender,
                (),
                ctx.services().edit_message(channel_id, msg_id, content),
                |_state, _data: (), res: Result<()>| { Ok(res?) }
            );

            Ok(fut)
        });

        methods.add_method("delete", |state, msg, (): ()| {
            let ctx = msg.0.bot.get_ctx();
            let channel_id = msg.channel().id();
            let msg_id = msg.0.id;

            let fut = create_lua_future!(
                state,
                msg.0.sender,
                (),
                ctx.services().delete_message(channel_id, msg_id),
                |_state, _data: (), res: Result<()>| { Ok(res?) }
            );

            Ok(fut)
        });

        methods.add_meta_method(MetaMethod::Index, |state, msg, index: String| {
            match index.as_str() {
                "id" => Ok(mlua::Value::String(
                    state.create_string(&msg.0.id.to_short_str())?,
                )),
                "author" => Ok(mlua::Value::UserData(
                    state.create_userdata(msg.author().clone())?,
                )),
                "content" => Ok(mlua::Value::String(
                    state.create_string(msg.0.content.as_bytes())?,
                )),
                "channel" => Ok(mlua::Value::UserData(
                    state.create_userdata(msg.channel().clone())?,
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
                nick: service_user.nick().to_string(),
                id: service_user.id(),
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
    nick: String,
    id: ServiceUserId,
    restricted: bool,
}

impl UserData for BotUser {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_meta_method(
            MetaMethod::Index,
            |state, user, index: String| match index.as_str() {
                "id" => Ok(mlua::Value::String(
                    state.create_string(user.0.id.to_str().as_bytes())?,
                )),
                "uid" => Ok(mlua::Value::Number(user.1.uid as f64)),
                "name" => Ok(mlua::Value::String(
                    state.create_string(user.0.name.as_bytes())?,
                )),
                "nick" => Ok(mlua::Value::String(
                    state.create_string(user.0.nick.as_bytes())?,
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
    bot: Arc<Bot>,
    sender: Sender<LuaAsyncCallback>,
    id: ChannelId,
    server: BotServer,
    service: ServiceKind,
}

impl BotChannel {
    pub async fn from_channel(
        bot: Arc<Bot>,
        sender: Sender<LuaAsyncCallback>,
        channel: &Arc<dyn Channel<impl Service>>,
    ) -> Result<BotChannel> {
        let service_server = channel.server().await? as Arc<dyn Server<_>>;
        let server = BotServer::from_server(&service_server).await?;

        Ok(BotChannel(Arc::new(BotChannelInner {
            bot,
            sender,
            id: channel.id(),
            server,
            service: channel.service().kind(),
        })))
    }

    pub fn id(&self) -> ChannelId {
        self.0.id
    }

    pub fn server(&self) -> &BotServer {
        &self.0.server
    }

    pub fn service(&self) -> ServiceKind {
        self.0.service
    }
}

impl UserData for BotChannel {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("escape_text", |_state, chan, text: String| {
            Ok(escape_untrusted_text(chan.0.service, text))
        });

        methods.add_method("supports_feature", |_state, chan, bits: u32| {
            Ok(chan
                .0
                .service
                .supports_feature(ServiceFeatures::from_bits_truncate(bits)))
        });

        methods.add_method("send", |state, chan, content: String| {
            let bot = chan.0.bot.clone();
            let sender = chan.0.sender.clone();
            let ctx = chan.0.bot.get_ctx();
            let channel_id = chan.id();

            let fut = create_lua_future!(
                state,
                chan.0.sender,
                (),
                async move {
                    match ctx.services().send_message(channel_id, content).await {
                        Ok(msg) => BotMessage::from_msg(bot, sender, &msg).await,
                        Err(err) => Err(err),
                    }
                },
                |_state, _data: (), res: Result<BotMessage>| { Ok(res?) }
            );

            Ok(fut)
        });

        methods.add_meta_method(
            MetaMethod::Index,
            |state, channel, index: String| match index.as_str() {
                "id" => Ok(mlua::Value::String(
                    state.create_string(&channel.0.id.to_short_str())?,
                )),
                "server" => Ok(mlua::Value::UserData(
                    state.create_userdata(channel.server().clone())?,
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
