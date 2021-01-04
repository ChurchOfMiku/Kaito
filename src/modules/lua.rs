use anyhow::Result;
use async_mutex::{Mutex, MutexGuardArc};
use std::{sync::Arc, time::Duration};

mod lib;
mod state;
mod utils;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    services::{Channel, ChannelId, Message, Server, ServerId, Service, User},
    settings::prelude::*,
};
use state::LuaState;

pub struct LuaModule {
    settings: Arc<LuaModuleSettings>,
    state: Arc<Mutex<LuaState>>,
}

settings! {
    LuaModuleSettings,
    {
        enable: bool => (true, SettingFlags::SERVER_OVERRIDE, "Enable the lua module", []),
        prefix: String => ("&".into(), SettingFlags::empty(), "Set the message prefix for lua commands", [max_len => 8]),
        always_eval: bool => (true, SettingFlags::SERVER_OVERRIDE, "Evaluate all messages in the sandbox", []),
        lua_prefix: String => ("]".into(), SettingFlags::empty(), "Set the lua prefix for runnning lua code in the sandbox with errors", [max_len => 8])
    }
}

#[async_trait]
impl Module for LuaModule {
    const KIND: ModuleKind = ModuleKind::Lua;
    const NAME: &'static str = "Lua";

    type ModuleConfig = ();
    type ModuleSettings = LuaModuleSettings;

    async fn load(bot: Arc<Bot>, _config: ()) -> Result<Arc<Self>> {
        Ok(Arc::new(LuaModule {
            settings: LuaModuleSettings::create()?,
            state: Arc::new(Mutex::new(LuaState::create_state(&bot)?)),
        }))
    }

    async fn message(&self, msg: Arc<dyn Message<impl Service>>) -> Result<()> {
        // Ignore the bot
        if msg.author().id() == msg.service().current_user().await?.id() {
            return Ok(());
        }

        // Get the channel and server
        let channel = msg.channel().await?;
        let server = channel.server().await?;

        // Find the command prefix for the channel
        let prefix = self
            .settings
            .prefix
            .value(server.id(), channel.id())
            .await?;

        let content = msg.content();

        // Check for command prefix
        match content.strip_prefix(&prefix) {
            Some(rest) => {
                let text = rest.to_string();
                return self.on_command(msg, text).await;
            }
            None => {}
        };

        let lua_prefix = self
            .settings
            .lua_prefix
            .value(server.id(), channel.id())
            .await?;

        match content.strip_prefix(&lua_prefix) {
            Some(rest) => {
                let text = rest.to_string();
                return self.eval_sandbox(msg, true, text).await;
            }
            None => {}
        };

        let text = content.to_string();
        self.eval_sandbox(msg, false, text).await
    }

    async fn enabled(&self, server_id: ServerId, channel_id: ChannelId) -> Result<bool> {
        self.settings.enable.value(server_id, channel_id).await
    }
}

impl LuaModule {
    async fn on_command(&self, _msg: Arc<dyn Message<impl Service>>, _rest: String) -> Result<()> {
        Ok(())
    }

    async fn eval_sandbox(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        _errors: bool,
        code: String,
    ) -> Result<()> {
        let lua_state = self.get_state().await?;

        let mut recv = lua_state.run_sandboxed(&code)?;

        while let Ok(out) = recv.try_next() {
            if let Some(out) = out {
                msg.channel().await?.send(out).await?;
            } else {
                tokio::time::delay_for(Duration::from_millis(100)).await;
            }
        }

        Ok(())
    }

    pub async fn get_state(&self) -> Result<MutexGuardArc<LuaState>> {
        Ok(self.state.lock_arc().await)
    }
}
