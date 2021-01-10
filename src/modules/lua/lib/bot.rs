use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{prelude::*, Error as LuaError, Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;
use thiserror::Error;

use super::{super::state::LuaAsyncCallback, r#async::create_future};
use crate::{
    bot::{Bot, ROLES},
    services::{Channel, ChannelId, Message, Service, ServiceKind, Services, User, UserId},
};

pub fn lib_bot(state: &Lua, bot: &Arc<Bot>, sender: Sender<LuaAsyncCallback>) -> Result<()> {
    let bot_tbl = state.create_table()?;

    let bot2 = bot.clone();
    let bot_restart_sandbox_fn = state.create_function(move |_, (): ()| {
        let ctx = bot2.get_ctx();
        tokio::spawn(async move {
            if let Err(err) = ctx.modules().lua.module().restart_sandbox().await {
                println!("error restarting sandbox: {}", err.to_string());
            }
        });

        Ok(())
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

        let (future_reg_key, fut) = wrap_future!(state, create_future(state));

        let sender = sender2.clone();
        tokio::spawn(async move {
            let res = match ctx.services().find_user(service, &user).await {
                Ok(user) => match ctx.bot().db().get_role_for_user(user.id()).await {
                    Ok(role) => ctx
                        .bot()
                        .db()
                        .is_restricted(user.id())
                        .await
                        .map(|restricted| (user, role, restricted)),
                    Err(err) => Err(err),
                },
                Err(err) => Err(err),
            };

            sender
                .send((
                    future_reg_key,
                    None,
                    Box::new(move |state| match &res {
                        Ok((user, role, restricted)) => {
                            Ok(LuaMultiValue::from_vec(vec![BotUser {
                                user_id: user.id(),
                                name: user.name().to_string(),
                                role: role.to_string(),
                                restricted: *restricted,
                            }
                            .to_lua(state)
                            .map_err(|e| e.to_string())?]))
                        }
                        Err(err) => Err(err.to_string()),
                    }),
                ))
                .unwrap();
        });

        Ok(fut)
    })?;
    bot_tbl.set("find_user", find_user_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let set_role_fn =
        state.create_function(move |state, (user, role): (LuaAnyUserData, String)| {
            let bot = bot2.clone();

            let user = user.borrow::<BotUser>()?.clone();

            let (future_reg_key, fut) = wrap_future!(state, create_future(state));

            let sender = sender2.clone();
            tokio::spawn(async move {
                let res = bot.db().set_role_for_user(user.user_id, &role).await;

                sender
                    .send((
                        future_reg_key,
                        None,
                        Box::new(move |_state| match &res {
                            Ok(_) => Ok(LuaMultiValue::new()),
                            Err(err) => Err(err.to_string()),
                        }),
                    ))
                    .unwrap();
            });

            Ok(fut)
        })?;
    bot_tbl.set("set_role", set_role_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let restrict_user_fn = state.create_function(
        move |state, (user, restrictor_id): (LuaAnyUserData, String)| {
            let bot = bot2.clone();

            let user = user.borrow::<BotUser>()?.clone();
            let restrictor_id = UserId::from_str(&restrictor_id).map_err(|_| {
                LuaError::ExternalError(Arc::new(BotError::InvalidUserId(restrictor_id)))
            })?;

            let (future_reg_key, fut) = wrap_future!(state, create_future(state));

            let sender = sender2.clone();
            tokio::spawn(async move {
                let res = bot.db().restrict_user(user.user_id, restrictor_id).await;

                sender
                    .send((
                        future_reg_key,
                        None,
                        Box::new(move |_state| match &res {
                            Ok(_) => Ok(LuaMultiValue::new()),
                            Err(err) => Err(err.to_string()),
                        }),
                    ))
                    .unwrap();
            });

            Ok(fut)
        },
    )?;
    bot_tbl.set("restrict_user", restrict_user_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let unrestrict_user_fn = state.create_function(move |state, (user,): (LuaAnyUserData,)| {
        let bot = bot2.clone();

        let user = user.borrow::<BotUser>()?.clone();

        let (future_reg_key, fut) = wrap_future!(state, create_future(state));

        let sender = sender2.clone();
        tokio::spawn(async move {
            let res = bot.db().unrestrict_user(user.user_id).await;

            sender
                .send((
                    future_reg_key,
                    None,
                    Box::new(move |_state| match &res {
                        Ok(_) => Ok(LuaMultiValue::new()),
                        Err(err) => Err(err.to_string()),
                    }),
                ))
                .unwrap();
        });

        Ok(fut)
    })?;
    bot_tbl.set("unrestrict_user", unrestrict_user_fn)?;

    bot_tbl.set("ROLES", ROLES)?;

    state.globals().set("bot", bot_tbl)?;

    Ok(())
}

pub struct BotMessage {
    bot: Arc<Bot>,
    user_id: UserId,
    channel_id: ChannelId,
    content: String,
    role: String,
    service: ServiceKind,
}

impl BotMessage {
    pub async fn from_msg(
        bot: Arc<Bot>,
        msg: &Arc<dyn Message<impl Service>>,
    ) -> Result<BotMessage> {
        let role = bot.db().get_role_for_user(msg.author().id()).await?;

        Ok(BotMessage {
            bot,
            user_id: msg.author().id(),
            channel_id: msg.channel().await?.id(),
            content: msg.content().to_string(),
            role,
            service: msg.service().kind(),
        })
    }

    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
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
                "content" => Ok(mlua::Value::String(
                    state.create_string(msg.content.as_bytes())?,
                )),
                "user_id" => Ok(mlua::Value::String(
                    state.create_string(msg.user_id.to_short_str().as_bytes())?,
                )),
                "role" => Ok(mlua::Value::String(
                    state.create_string(msg.role.as_bytes())?,
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
    user_id: UserId,
    name: String,
    role: String,
    restricted: bool,
}

impl UserData for BotUser {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_meta_method(
            MetaMethod::Index,
            |state, user, index: String| match index.as_str() {
                "id" => Ok(mlua::Value::String(
                    state.create_string(user.user_id.to_short_str().as_bytes())?,
                )),
                "name" => Ok(mlua::Value::String(
                    state.create_string(user.name.as_bytes())?,
                )),
                "role" => Ok(mlua::Value::String(
                    state.create_string(user.role.as_bytes())?,
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
    #[error("invalid user id \"{}\"", _0)]
    InvalidUserId(String),
}
