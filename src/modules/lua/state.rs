use anyhow::Result;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use mlua::{Function, Lua, StdLib, Table, UserData, UserDataMethods};
use std::sync::Arc;

use super::lib::{include_lua, lib_include, os::lib_os};
use crate::bot::Bot;

pub struct LuaState {
    inner: Lua,
}

impl LuaState {
    pub fn create_state(bot: &Arc<Bot>) -> Result<LuaState> {
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

        include_lua(&inner, &lua_root_path, "init.lua")?;

        Ok(LuaState { inner })
    }

    pub fn run_sandboxed(&self, source: &str) -> Result<UnboundedReceiver<String>> {
        let sandbox_tbl: Table = self.inner.globals().get("sandbox")?;
        let run_fn: Function = sandbox_tbl.get("run")?;

        let (sender, receiver) = unbounded();

        run_fn.call((SandboxState { sender }, source))?;

        Ok(receiver)
    }
}

pub struct SandboxState {
    sender: UnboundedSender<String>,
}

impl UserData for SandboxState {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("print", |_, this, value: String| {
            this.sender.unbounded_send(value).ok(); // Ignore the error for now
            Ok(())
        });
    }
}
