use anyhow::Result;
use crossbeam::channel::{unbounded, Receiver, Sender};
use mlua::{
    prelude::{LuaError, LuaMultiValue, LuaValue},
    Function, Lua, RegistryKey, StdLib, Table, ToLua, UserData, UserDataMethods,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use super::lib::{include_lua, lib_include, os::lib_os, r#async::lib_async};
use crate::bot::Bot;

pub type LuaAsyncCallback = (
    RegistryKey,
    Option<SandboxState>,
    Box<dyn Fn(&Lua) -> LuaMultiValue + Send + Sync>,
);

pub struct LuaState {
    inner: Lua,
    sandbox: bool,
    async_receiver: Receiver<LuaAsyncCallback>,
}

impl LuaState {
    pub fn create_state(bot: &Arc<Bot>, sandbox: bool) -> Result<LuaState> {
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

        let (sender, async_receiver) = unbounded();

        lib_async(&inner, sender)?;
        lib_os(&inner)?;

        let lua_root_path = bot.root_path().join("lua");

        lib_include(lua_root_path.clone(), &inner)?;

        if sandbox {
            include_lua(&inner, &lua_root_path, "sandbox.lua")?;
        } else {
            include_lua(&inner, &lua_root_path, "bot.lua")?;
        }

        // Limit memory to 256 MiB
        inner.set_memory_limit(256 * 1024 * 1024)?;

        Ok(LuaState {
            inner,
            sandbox,
            async_receiver,
        })
    }

    pub fn run_sandboxed(&self, source: &str) -> Result<Receiver<SandboxMsg>> {
        let sandbox_tbl: Table = self.inner.globals().get("sandbox")?;
        let run_fn: Function = sandbox_tbl.get("run")?;

        let (sender, receiver) = unbounded();

        let sandbox_state = SandboxState(Arc::new(SandboxStateInner {
            sender: sender.clone(),
            instructions_run: AtomicU64::new(0),
        }));

        self.inner
            .set_named_registry_value("__SANDBOX_STATE", sandbox_state.clone())?;

        run_fn.call((sandbox_state, source))?;

        Ok(receiver)
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
        }

        loop {
            match self.async_receiver.try_recv() {
                Ok((fut_reg_key, sandbox_state, cb)) => {
                    let value = cb(&self.inner);
                    let future: Table = self.inner.registry_value(&fut_reg_key)?;

                    let resolve_fn: Function = future.get("__handle_resolve")?;

                    let succ = true;

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
                                        LuaValue::Boolean(succ),
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
                                vec![
                                    LuaValue::Table(future.clone()),
                                    LuaValue::Boolean(true),
                                    LuaValue::Boolean(succ),
                                ],
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
}

pub enum SandboxMsg {
    Out(String),
    Error(String),
    Terminated(SandboxTerminationReason),
}

pub enum SandboxTerminationReason {
    ExecutionQuota,
    Ended,
}

#[derive(Clone)]
pub struct SandboxState(Arc<SandboxStateInner>);

pub struct SandboxStateInner {
    sender: Sender<SandboxMsg>,
    instructions_run: AtomicU64,
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

        methods.add_method("terminate", |_, this, value: String| {
            let reason = match value.as_ref() {
                "exec" => SandboxTerminationReason::ExecutionQuota,
                "" => SandboxTerminationReason::Ended,
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
