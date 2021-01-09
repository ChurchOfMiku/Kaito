use anyhow::Result;
use mlua::{Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;

use crate::{
    bot::{Bot, ROLES},
    services::{Channel, ChannelId, Message, Service, User},
};

pub fn lib_bot(state: &Lua, bot: &Arc<Bot>) -> Result<()> {
    let bot_tbl = state.create_table()?;

    let bot = bot.clone();
    let bot_restart_sandbox_fn = state.create_function(move |_, (): ()| {
        let ctx = bot.get_ctx();
        tokio::spawn(async move {
            if let Err(err) = ctx.modules().lua.module().restart_sandbox().await {
                println!("error restarting sandbox: {}", err.to_string());
            }
        });

        Ok(())
    })?;
    bot_tbl.set("restart_sandbox", bot_restart_sandbox_fn)?;

    bot_tbl.set("ROLES", ROLES)?;

    state.globals().set("bot", bot_tbl)?;

    Ok(())
}

pub struct BotMessage {
    bot: Arc<Bot>,
    channel_id: ChannelId,
    content: String,
    role: String,
}

impl BotMessage {
    pub async fn from_msg(
        bot: Arc<Bot>,
        msg: &Arc<dyn Message<impl Service>>,
    ) -> Result<BotMessage> {
        let role = bot.db().get_role_for_user(msg.author().id()).await?;

        Ok(BotMessage {
            bot,
            channel_id: msg.channel().await?.id(),
            content: msg.content().to_string(),
            role,
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
                "role" => Ok(mlua::Value::String(
                    state.create_string(msg.role.as_bytes())?,
                )),
                _ => Ok(mlua::Value::Nil),
            }
        });
    }
}
