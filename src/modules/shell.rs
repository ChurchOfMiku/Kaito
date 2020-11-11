use anyhow::Result;
use std::sync::Arc;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    services::{Channel, ChannelId, Message, Server, ServerId, Service},
    settings::prelude::*,
};

pub struct ShellModule {
    settings: Arc<ShellModuleSettings>,
}

settings! {
    ShellModuleSettings,
    {
        enable: bool => (true, SettingFlags::SERVER_OVERRIDE, "Enable the shell module", []),
        prefix: String => ("$".into(), SettingFlags::empty(), "Set the message prefix for shell commands", [max_len => 8])
    }
}

#[async_trait]
impl Module for ShellModule {
    const KIND: ModuleKind = ModuleKind::Shell;
    const NAME: &'static str = "Shell";

    type ModuleConfig = ();
    type ModuleSettings = ShellModuleSettings;

    async fn load(_bot: Arc<Bot>, _config: ()) -> Result<Arc<Self>> {
        Ok(Arc::new(ShellModule {
            settings: ShellModuleSettings::create()?,
        }))
    }

    async fn message(&self, msg: Arc<dyn Message<impl Service>>) -> Result<()> {
        // Get the channel and server
        let channel = msg.channel().await?;
        let server = channel.server().await?;

        // Find the shell prefix for the channel
        let prefix = self.settings.prefix.value(server.id(), channel.id());

        let content = msg.content();

        let _rest = match content.strip_prefix(&prefix) {
            Some(rest) => rest,
            None => return Ok(()),
        };

        Ok(())
    }

    fn enabled(&self, server_id: ServerId, channel_id: ChannelId) -> bool {
        self.settings.enable.value(server_id, channel_id)
    }
}
