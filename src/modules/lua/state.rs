use anyhow::Result;
use crossbeam::channel::{unbounded, Receiver, Sender};
use mlua::{prelude::LuaError, Function, Lua, StdLib, Table, UserData, UserDataMethods};
use std::sync::Arc;

use super::lib::{include_lua, lib_include, os::lib_os};
use crate::bot::Bot;

pub struct LuaState {
    inner: Lua,
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

        lib_os(&inner)?;

        let lua_root_path = bot.root_path().join("lua");

        lib_include(lua_root_path.clone(), &inner)?;

        if sandbox {
            include_lua(&inner, &lua_root_path, "sandbox.lua")?;
        } else {
            include_lua(&inner, &lua_root_path, "init.lua")?;
        }

        // Limit memory to 256 MiB
        inner.set_memory_limit(256 * 1024 * 1024)?;

        Ok(LuaState { inner })
    }

    pub fn run_sandboxed(&self, source: &str) -> Result<Receiver<SandboxMsg>> {
        let sandbox_tbl: Table = self.inner.globals().get("sandbox")?;
        let run_fn: Function = sandbox_tbl.get("run")?;

        let (sender, receiver) = unbounded();

        run_fn.call((SandboxState { sender }, source))?;

        Ok(receiver)
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

pub struct SandboxState {
    sender: Sender<SandboxMsg>,
}

impl UserData for SandboxState {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("print", |_, this, value: String| {
            this.sender.send(SandboxMsg::Out(value)).ok(); // Ignore the error for now
            Ok(())
        });

        methods.add_method("error", |_, this, value: String| {
            this.sender.send(SandboxMsg::Error(value)).ok(); // Ignore the error for now
            Ok(())
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

            this.sender.send(SandboxMsg::Terminated(reason)).ok();

            Ok(())
        });
    }
}
