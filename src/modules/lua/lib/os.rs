use anyhow::Result;
use mlua::Lua;

use super::super::utils::get_duration;

pub fn lib_os(state: &Lua) -> Result<()> {
    let os = state.create_table()?;

    // os.clock
    let os_clock = state.create_function(|_, ()| Ok(get_duration()))?;
    os.set("clock", os_clock)?;

    state.globals().set("os", os)?;

    Ok(())
}
