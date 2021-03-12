use anyhow::Result;
use async_mutex::{Mutex, MutexGuardArc};
use crossbeam::channel::TryRecvError;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[macro_use]
mod lib;
mod http;
mod state;
mod utils;

use self::lib::bot::BotUser;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    services::{
        Channel, ChannelId, Message, Server, ServerId, Service, ServiceFeatures, ServiceKind, User,
    },
    settings::prelude::*,
    utils::{escape_untrusted_text, shell_parser::parse_shell_args},
};
use lib::bot::BotMessage;
use state::{LuaState, SandboxMsg, SandboxTerminationReason};

pub struct LuaModule {
    bot: Arc<Bot>,
    settings: Arc<LuaModuleSettings>,
    bot_state: Arc<Mutex<LuaState>>,
    sandbox_state: Arc<Mutex<LuaState>>,
}

settings! {
    LuaModuleSettings,
    LuaModule,
    {
        enable: bool => (true, SettingFlags::empty(), "Enable the lua module", []),
        prefix: String => ("&".into(), SettingFlags::empty(), "Set the message prefix for lua commands", [max_len => 8]),
        always_eval: bool => (false, SettingFlags::empty(), "Evaluate all messages in the sandbox", []),
        lua_prefix: String => ("]".into(), SettingFlags::empty(), "Set the lua prefix for runnning lua code in the sandbox with errors", [max_len => 8])
    }
}

#[async_trait]
impl Module for LuaModule {
    const KIND: ModuleKind = ModuleKind::Lua;
    const ID: &'static str = "lua";
    const NAME: &'static str = "Lua";

    type ModuleConfig = ();
    type ModuleSettings = LuaModuleSettings;

    async fn load(bot: Arc<Bot>, _config: ()) -> Result<Arc<LuaModule>> {
        let sandbox_state = Arc::new(Mutex::new(LuaState::create_state(&bot, true, None)?));
        let bot_state = Arc::new(Mutex::new(LuaState::create_state(
            &bot,
            false,
            Some(sandbox_state.clone()),
        )?));

        let bot_state2 = bot_state.clone();
        let sandbox_state2 = sandbox_state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));

            loop {
                interval.tick().await;
                if let Err(err) = bot_state2.lock_arc().await.think() {
                    println!("error: {}", err.to_string());
                }
                if let Err(err) = sandbox_state2.lock_arc().await.think() {
                    println!("error: {}", err.to_string());
                }
            }
        });

        Ok(Arc::new(LuaModule {
            bot: bot.clone(),
            settings: LuaModuleSettings::create(bot)?,
            bot_state,
            sandbox_state,
        }))
    }

    async fn message(&self, msg: Arc<dyn Message<impl Service>>) -> Result<()> {
        // Ignore the bot
        if msg.author().id() == msg.service().current_user().await?.id()
            || msg.author().bot() == Some(true)
        {
            return Ok(());
        }

        let user = self
            .bot
            .db()
            .get_user_from_service_user_id(msg.author().id())
            .await?;

        if self.bot.db().is_restricted(user.uid).await? {
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

        {
            let lua_state = self.get_bot_state().await?;
            let sender = lua_state.async_sender();

            let bot_msg = BotMessage::from_msg(self.bot.clone(), sender, &msg).await?;
            lua_state.run_bot_message(bot_msg)?;
        }

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

        if self
            .settings
            .always_eval
            .value(server.id(), channel.id())
            .await?
        {
            let text = content.to_string();
            self.eval_sandbox(msg, false, text).await
        } else {
            Ok(())
        }
    }

    async fn reaction(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        reactor: Arc<dyn User<impl Service>>,
        reaction: String,
        remove: bool,
    ) -> Result<()> {
        let lua_state = self.get_bot_state().await?;
        let sender = lua_state.async_sender();

        let bot_msg = BotMessage::from_msg(self.bot.clone(), sender, &msg).await?;
        let bot_reactor = BotUser::from_user(self.bot.clone(), &reactor).await?;

        lua_state.run_bot_reaction(bot_msg, bot_reactor, reaction, remove)?;

        Ok(())
    }

    async fn enabled(&self, server_id: ServerId, channel_id: ChannelId) -> Result<bool> {
        self.settings.enable.value(server_id, channel_id).await
    }

    fn settings(&self) -> &Arc<LuaModuleSettings> {
        &self.settings
    }
}

impl LuaModule {
    async fn on_command(&self, msg: Arc<dyn Message<impl Service>>, rest: String) -> Result<()> {
        let args = parse_shell_args(
            msg.service()
                .kind()
                .supports_feature(ServiceFeatures::MARKDOWN),
            &rest,
        )?;

        let lua_state = self.get_bot_state().await?;
        let sender = lua_state.async_sender();
        let bot_msg = BotMessage::from_msg(self.bot.clone(), sender, &msg).await?;

        let res = lua_state.run_bot_command(bot_msg, args);
        drop(lua_state);

        if let Err(err) = res {
            msg.channel().await?.send(err.to_string()).await?;
        }

        Ok(())
    }

    async fn restart_sandbox(&self) -> Result<()> {
        *self.get_sandbox_state().await? = LuaState::create_state(&self.bot, true, None)?;

        Ok(())
    }

    async fn eval_sandbox(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        errors: bool,
        code: String,
    ) -> Result<()> {
        let code = trim_codeblocks(msg.service().kind(), code);

        let lua_state = self.get_sandbox_state().await?;

        let (sandbox_state, recv) = match lua_state.run_sandboxed(&code, None) {
            Ok(recv) => recv,
            Err(_err) => {
                return Ok(());
            }
        };

        drop(lua_state);

        let mut buffer: Vec<String> = Vec::new();
        let mut last_msg = Instant::now();
        let mut has_messaged = false; // only wait 100ms for the first message
        let mut aborting = None;

        while aborting.is_none() {
            match recv.try_recv() {
                Ok(out) => match out {
                    SandboxMsg::Out(out) => {
                        if !out.is_empty() {
                            let mut lines =
                                out.split('\n').map(|l| l.to_string()).collect::<Vec<_>>();

                            let lines_left = sandbox_state.limits.lines_left();

                            if lines_left > 0 {
                                buffer.append(&mut lines);
                                sandbox_state.limits.set_lines_left(lines_left - 1);
                            } else {
                                aborting = Some("error: too many lines has been output, aborting");
                            }
                        }
                    }
                    SandboxMsg::Error(err) => {
                        if errors && !err.is_empty() {
                            msg.channel()
                                .await?
                                .send(escape_untrusted_text(
                                    msg.service().kind(),
                                    format!("error: {}", err),
                                ))
                                .await?;
                        }
                    }
                    SandboxMsg::Terminated(reason) => match reason {
                        SandboxTerminationReason::Done => {}
                        SandboxTerminationReason::ExecutionQuota => {
                            msg.channel()
                                .await?
                                .send("Execution quota exceeded, terminated execution")
                                .await?;
                            break;
                        }
                        SandboxTerminationReason::TimeLimit => {
                            msg.channel()
                                .await?
                                .send("Execution time limit reached, terminated execution")
                                .await?;
                            break;
                        }
                    },
                },
                Err(TryRecvError::Empty) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(TryRecvError::Disconnected) => break,
            }

            // Empty the buffer
            if !buffer.is_empty() {
                let elapsed = last_msg.elapsed();
                let wait = if has_messaged {
                    Duration::from_millis(500)
                } else {
                    Duration::from_millis(100)
                };

                if elapsed > wait || aborting.is_some() {
                    let mut out = String::new();

                    let mut characters_left = sandbox_state.limits.characters_left();

                    let mut lines = buffer.drain(..).peekable();
                    while let Some(line) = lines.next() {
                        let len = line.len() as u64;

                        if characters_left > len {
                            characters_left -= len;
                            out.push_str(&escape_untrusted_text(msg.service().kind(), line));

                            if lines.peek().is_some() {
                                out.push_str("\n");
                            }
                        } else {
                            aborting = Some("error: too many characters has been output, aborting");
                            break;
                        }
                    }

                    sandbox_state.limits.set_characters_left(characters_left);

                    msg.channel().await?.send(out).await?;

                    last_msg = Instant::now();
                    has_messaged = true;
                }
            }
        }

        if let Some(aborting) = aborting {
            msg.channel().await?.send(aborting).await?;
        }

        Ok(())
    }

    pub async fn get_bot_state(&self) -> Result<MutexGuardArc<LuaState>> {
        Ok(self.bot_state.lock_arc().await)
    }

    pub async fn get_sandbox_state(&self) -> Result<MutexGuardArc<LuaState>> {
        Ok(self.sandbox_state.lock_arc().await)
    }
}

fn trim_codeblocks(service: ServiceKind, text: String) -> String {
    let trimmed = text.trim();

    match service {
        ServiceKind::Discord => {
            if let Some(inside) = trimmed
                .strip_prefix("```lua\n")
                .or_else(|| trimmed.strip_prefix("```"))
                .and_then(|s| s.strip_suffix("```"))
            {
                inside.trim().to_string()
            } else {
                text
            }
        }
        #[allow(unreachable_patterns)]
        _ => text,
    }
}
