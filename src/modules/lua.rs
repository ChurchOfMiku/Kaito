use anyhow::Result;
use std::sync::Arc;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    services::{Channel, ChannelId, Message, Server, ServerId, Service},
    settings::prelude::*,
};

pub struct LuaModule {
    settings: Arc<LuaModuleSettings>,
}

settings! {
    LuaModuleSettings,
    {
        enable: bool => (true, SettingFlags::SERVER_OVERRIDE, "Enable the lua module", []),
        prefix: String => ("&".into(), SettingFlags::empty(), "Set the message prefix for lua commands", [max_len => 8])
    }
}

#[async_trait]
impl Module for LuaModule {
    const KIND: ModuleKind = ModuleKind::Lua;
    const NAME: &'static str = "Lua";

    type ModuleConfig = ();
    type ModuleSettings = LuaModuleSettings;

    async fn load(_bot: Arc<Bot>, _config: ()) -> Result<Arc<Self>> {
        Ok(Arc::new(LuaModule {
            settings: LuaModuleSettings::create()?,
        }))
    }

    async fn message(&self, msg: Arc<dyn Message<impl Service>>) -> Result<()> {
        // Get the channel and server
        let channel = msg.channel().await?;
        let server = channel.server().await?;

        // Find the lua prefix for the channel
        let prefix = self
            .settings
            .prefix
            .value(server.id(), channel.id())
            .await?;

        let content = msg.content();

        let _rest = match content.strip_prefix(&prefix) {
            Some(rest) => rest,
            None => return Ok(()),
        };

        Ok(())
    }

    async fn enabled(&self, server_id: ServerId, channel_id: ChannelId) -> Result<bool> {
        self.settings.enable.value(server_id, channel_id).await
    }
}
