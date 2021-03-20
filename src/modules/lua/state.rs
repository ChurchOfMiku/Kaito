use anyhow::Result;
use async_mutex::Mutex;
use crossbeam::channel::{unbounded, Receiver, Sender};
use governor::{
    clock::QuantaClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use mlua::{
    prelude::{LuaError, LuaMultiValue, LuaValue},
    Function, Lua, LuaSerdeExt, RegistryKey, StdLib, Table, Thread, ThreadStatus, ToLua, UserData,
    UserDataMethods,
};
use paste::paste;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use super::{
    http,
    lib::{
        bot::{lib_bot, BotMessage, BotUser},
        include_lua, lib_include,
        os::lib_os,
        r#async::lib_async,
        tags::lib_tags,
    },
};
use crate::{
    bot::Bot, message::MessageSettings, services::ChannelId, utils::escape_untrusted_text,
};

pub type LuaAsyncCallback = (
    RegistryKey,
    Option<SandboxState>,
    Box<dyn FnOnce(&Lua) -> Result<LuaMultiValue> + Send>,
);

macro_rules! atomic_get_set {
    ($ident:ident, $ty:ty) => {
        paste! {
            pub fn[<set_ $ident>](&self, value: $ty) {
                self.$ident.store(value, Ordering::Relaxed)
            }

            pub fn $ident(&self) -> $ty {
                self.$ident.load(Ordering::Relaxed)
            }
        }
    };
}

pub struct LuaState {
    bot: Arc<Bot>,
    inner: Lua,
    sandbox: bool,
    async_sender: Sender<LuaAsyncCallback>,
    async_receiver: Receiver<LuaAsyncCallback>,
    http_rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
    thread_id: Arc<AtomicU64>,
    shutting_down: AtomicBool,
}

impl LuaState {
    pub fn create_state(
        bot: &Arc<Bot>,
        sandbox: bool,
        sandbox_state: Option<Arc<Mutex<LuaState>>>,
    ) -> Result<LuaState> {
        // Avoid loading os and io
        let inner = unsafe {
            Lua::unsafe_new_with(
                StdLib::COROUTINE
                    | StdLib::TABLE
                    | StdLib::STRING
                    | StdLib::UTF8
                    | StdLib::MATH
                    | StdLib::DEBUG,
            )
        };

        let (async_sender, async_receiver) = unbounded();

        let thread_id = Arc::new(AtomicU64::new(0));

        lib_async(&inner, async_sender.clone(), thread_id.clone())?;
        lib_os(&inner)?;

        let lua_root_path = bot.share_path().join("lua");

        lib_include(lua_root_path.clone(), &inner)?;

        if sandbox {
            include_lua(&inner, &lua_root_path, "sandbox.lua")?;
        } else {
            lib_bot(
                &inner,
                bot,
                async_sender.clone(),
                sandbox_state.expect("sandbox state for bot state"),
            )?;
            http::lib_http(&inner, async_sender.clone())?;
            lib_tags(&inner, bot, async_sender.clone())?;
            inner.set_named_registry_value("__ASYNC_THREADS", inner.create_table()?)?;
            inner.set_named_registry_value("__ASYNC_THREADS_CHANNELS", inner.create_table()?)?;
            include_lua(&inner, &lua_root_path, "bot.lua")?;
        }

        // Limit memory to 256 MiB
        inner.set_memory_limit(256 * 1024 * 1024)?;

        let http_rate_limiter = Arc::new(RateLimiter::direct(Quota::per_second(
            std::num::NonZeroU32::new(2).unwrap(),
        )));

        Ok(LuaState {
            bot: bot.clone(),
            inner,
            sandbox,
            async_sender,
            async_receiver,
            http_rate_limiter,
            thread_id,
            shutting_down: AtomicBool::new(false),
        })
    }

    fn create_async_thread(&self, thread: Thread, channel_id: Option<ChannelId>) -> Result<()> {
        if thread.status() == ThreadStatus::Resumable {
            let threads: Table = self.inner.named_registry_value("__ASYNC_THREADS")?;
            let thread_channels: Table = self
                .inner
                .named_registry_value("__ASYNC_THREADS_CHANNELS")?;
            let id = self.thread_id.fetch_add(1, Ordering::AcqRel);
            threads.set(id, thread)?;
            if let Some(channel_id) = channel_id {
                thread_channels.set(id, channel_id.to_short_str())?;
            }
        }

        Ok(())
    }

    pub fn run_bot_command(&self, msg: BotMessage, args: Vec<String>) -> Result<()> {
        let sandbox_tbl: Table = self.inner.globals().get("bot")?;
        let on_command_fn: Function = sandbox_tbl.get("on_command")?;

        let thread = self.inner.create_thread(on_command_fn)?;
        let channel_id = msg.channel().id();
        thread.resume((msg, args))?;

        self.create_async_thread(thread, Some(channel_id))?;

        Ok(())
    }

    pub fn run_bot_message(&self, msg: BotMessage) -> Result<()> {
        let sandbox_tbl: Table = self.inner.globals().get("bot")?;
        let on_message_fn: Function = sandbox_tbl.get("on_message")?;

        let thread = self.inner.create_thread(on_message_fn)?;
        let channel_id = msg.channel().id();
        thread.resume(msg)?;

        self.create_async_thread(thread, Some(channel_id))?;

        Ok(())
    }

    pub fn run_bot_reaction(
        &self,
        msg: BotMessage,
        reactor: BotUser,
        reaction: String,
        removed: bool,
    ) -> Result<()> {
        let sandbox_tbl: Table = self.inner.globals().get("bot")?;
        let on_reaction_fn: Function = sandbox_tbl.get("on_reaction")?;

        let thread = self.inner.create_thread(on_reaction_fn)?;
        let channel_id = msg.channel().id();
        thread.resume((msg, reactor, reaction, removed))?;

        self.create_async_thread(thread, Some(channel_id))?;

        Ok(())
    }

    pub fn run_sandboxed(
        &self,
        source: &str,
        env_encoded: Option<String>,
    ) -> Result<(Arc<SandboxStateInner>, Receiver<SandboxMsg>)> {
        let sandbox_tbl: Table = self.inner.globals().get("sandbox")?;
        let run_fn: Function = sandbox_tbl.get("run")?;

        let (sender, receiver) = unbounded();

        let sandbox_state = SandboxState(Arc::new(SandboxStateInner {
            async_sender: self.async_sender.clone(),
            sender: sender.clone(),
            instructions_run: AtomicU64::new(0),
            limits: SandboxLimits {
                lines_left: AtomicU64::new(10),
                characters_left: AtomicU64::new(2000),
                http_calls_left: AtomicU64::new(2),
                instructions: 262144,
            },
            http_rate_limiter: self.http_rate_limiter.clone(),
        }));

        self.inner
            .set_named_registry_value("__SANDBOX_STATE", sandbox_state.clone())?;

        if let Some(env_encoded) = env_encoded {
            let env = self.inner.to_value(&env_encoded)?;
            run_fn.call((sandbox_state.clone(), source, env, true))?;
        } else {
            run_fn.call((sandbox_state.clone(), source, LuaValue::Nil, true))?;
        }

        Ok((sandbox_state.0, receiver))
    }

    pub fn think(&self) -> Result<()> {
        if self.sandbox {
            let sandbox_tbl: Table = self.inner.globals().get("sandbox")?;
            let think_fn: Function = sandbox_tbl.get("think")?;
            think_fn.call(())?;
        } else {
            let bot_tbl: Table = self.inner.globals().get("bot")?;
            let think_fn: Function = bot_tbl.get("think")?;
            think_fn.call(())?;

            let threads: Table = self.inner.named_registry_value("__ASYNC_THREADS")?;
            let thread_channels: Table = self
                .inner
                .named_registry_value("__ASYNC_THREADS_CHANNELS")?;

            for pair in threads.clone().pairs::<u64, Thread>() {
                let (id, thread) = pair?;

                if let Err(err) = thread.resume::<_, ()>(()) {
                    if let Ok(channel_str) = thread_channels.get::<u64, String>(id) {
                        let id = ChannelId::from_str(&channel_str)?;
                        let bot = self.bot.clone();

                        tokio::spawn(async move {
                            bot.get_ctx()
                                .services()
                                .send_message(
                                    id,
                                    escape_untrusted_text(id.service_kind(), err.to_string()),
                                    MessageSettings::default(),
                                )
                                .await
                                .ok();
                        });
                    } else {
                        println!("error during bot async think: {}", err.to_string());
                    }
                }

                if thread.status() != ThreadStatus::Resumable {
                    threads.set(id, LuaValue::Nil)?;
                    thread_channels.set(id, LuaValue::Nil)?;
                }
            }
        }

        self.think_async_callbacks()?;

        Ok(())
    }

    fn think_async_callbacks(&self) -> Result<()> {
        loop {
            // Check for async callbacks
            match self.async_receiver.try_recv() {
                Ok((fut_reg_key, sandbox_state, cb)) => {
                    let (succ, value) = match cb(&self.inner) {
                        Ok(vals) => (true, vals),
                        Err(err) => (
                            false,
                            LuaMultiValue::from_vec(vec![LuaValue::String(
                                self.inner.create_string(&err.to_string())?,
                            )]),
                        ),
                    };
                    let future: Table = self.inner.registry_value(&fut_reg_key)?;
                    let resolve_fn: Function = if succ {
                        future.get("__handle_resolve")?
                    } else {
                        future.get("__handle_reject")?
                    };

                    // Sandbox when resolving the future
                    if self.sandbox {
                        if let Some(sandbox_state) = sandbox_state {
                            let sandbox_tbl: Table = self.inner.globals().get("sandbox")?;
                            let run_fn: Function = sandbox_tbl.get("async_callback")?;

                            let args = LuaMultiValue::from_vec(
                                [
                                    vec![
                                        sandbox_state.to_lua(&self.inner)?,
                                        LuaValue::Table(future.clone()),
                                        LuaValue::Boolean(true),
                                    ],
                                    value.into_vec(),
                                ]
                                .concat(),
                            );

                            run_fn.call::<_, ()>(args)?;
                        }
                    } else {
                        let args = LuaMultiValue::from_vec(
                            [
                                vec![LuaValue::Table(future.clone()), LuaValue::Boolean(true)],
                                value.into_vec(),
                            ]
                            .concat(),
                        );

                        resolve_fn.call::<_, ()>(args)?;
                    }

                    // Clean up the async registry values
                    self.inner.remove_registry_value(fut_reg_key)?;
                }
                _ => break,
            }
        }

        Ok(())
    }

    pub fn on_loaded(&self) -> Result<()> {
        if !self.sandbox {
            let bot_tbl: Table = self.inner.globals().get("bot")?;
            let on_loaded_fn: Function = bot_tbl.get("on_loaded")?;

            let thread = self.inner.create_thread(on_loaded_fn)?;
            thread.resume(())?;

            self.create_async_thread(thread, None)?;
        }

        Ok(())
    }

    pub fn shutdown(&self) -> Result<bool> {
        if !self.sandbox {
            if self.shutting_down.swap(true, Ordering::Relaxed) {
                self.think_async_callbacks()?;

                let thread: Thread = self.inner.named_registry_value("__ASYNC_SHUTDOWN_THREAD")?;

                if thread.status() != ThreadStatus::Resumable {
                    return Ok(false);
                }

                thread.resume(())?;
            } else {
                let bot_tbl: Table = self.inner.globals().get("bot")?;
                let shutdown_fn: Function = bot_tbl.get("shutdown")?;
                let thread = self.inner.create_thread(shutdown_fn)?;
                thread.resume(())?;
                self.inner
                    .set_named_registry_value("__ASYNC_SHUTDOWN_THREAD", thread)?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn async_sender(&self) -> Sender<LuaAsyncCallback> {
        self.async_sender.clone()
    }
}

pub enum SandboxMsg {
    Out(String),
    Error(String),
    Terminated(SandboxTerminationReason),
}

pub enum SandboxTerminationReason {
    Done,
    ExecutionQuota,
    TimeLimit,
}

#[derive(Clone)]
pub struct SandboxState(pub Arc<SandboxStateInner>);

pub struct SandboxStateInner {
    pub async_sender: Sender<LuaAsyncCallback>,
    pub sender: Sender<SandboxMsg>,
    pub instructions_run: AtomicU64,
    pub limits: SandboxLimits,
    pub http_rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
}

pub struct SandboxLimits {
    pub lines_left: AtomicU64,
    pub characters_left: AtomicU64,
    pub http_calls_left: AtomicU64,
    pub instructions: u64,
}

impl SandboxLimits {
    atomic_get_set! {lines_left, u64}
    atomic_get_set! {characters_left, u64}
}

impl UserData for SandboxState {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("print", |_, this, value: String| {
            this.0.sender.send(SandboxMsg::Out(value)).ok(); // Ignore the error for now
            Ok(())
        });

        methods.add_method("error", |_, this, value: String| {
            this.0.sender.send(SandboxMsg::Error(value)).ok(); // Ignore the error for now
            Ok(())
        });

        methods.add_method("set_instructions_run", |_, this, value: u64| {
            this.0.instructions_run.store(value, Ordering::Relaxed);
            Ok(())
        });

        methods.add_method("get_instructions_run", |_, this, _: ()| {
            Ok(this.0.instructions_run.load(Ordering::Relaxed))
        });

        methods.add_method("get_instruction_limit", |_, this, _: ()| {
            Ok(this.0.limits.instructions)
        });

        methods.add_method("set_state", |state, this, _: ()| {
            state.set_named_registry_value("__SANDBOX_STATE", this.clone())?;
            Ok(())
        });

        methods.add_method(
            "http_fetch",
            |state, this, (url, options): (String, Table)| {
                http::http_fetch(state, this, &url, options)
            },
        );

        methods.add_method("terminate", |_, this, value: String| {
            let reason = match value.as_ref() {
                "done" => SandboxTerminationReason::Done,
                "exec" => SandboxTerminationReason::ExecutionQuota,
                "time" => SandboxTerminationReason::TimeLimit,
                _ => {
                    return Err(LuaError::RuntimeError(format!(
                        "unknown termination reason: \"{}\"",
                        value
                    )))
                }
            };

            this.0.sender.send(SandboxMsg::Terminated(reason)).ok();

            Ok(())
        });
    }
}
