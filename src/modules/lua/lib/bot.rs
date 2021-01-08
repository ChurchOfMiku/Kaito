use anyhow::Result;
use mlua::{Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;

use crate::{
    bot::Bot,
    services::{Channel, ChannelId, Message, Service},
};

pub fn lib_bot(state: &Lua) -> Result<()> {
    let bot_tbl = state.create_table()?;

    state.globals().set("bot", bot_tbl)?;

    Ok(())
}

pub struct BotMessage {
    bot: Arc<Bot>,
    channel_id: ChannelId,
    content: String,
}

impl BotMessage {
    pub async fn from_msg(
        bot: Arc<Bot>,
        msg: &Arc<dyn Message<impl Service>>,
    ) -> Result<BotMessage> {
        Ok(BotMessage {
            bot,
            channel_id: msg.channel().await?.id(),
            content: msg.content().to_string(),
        })
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
                _ => Ok(mlua::Value::Nil),
            }
        });
    }
}
