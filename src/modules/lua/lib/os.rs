use anyhow::Result;
use mlua::{Error, Lua};
use std::{sync::Arc, time::SystemTime};

use super::super::utils::get_duration;

pub fn lib_os(state: &Lua) -> Result<()> {
    let os = state.create_table()?;

    // os.clock
    let os_clock = state.create_function(|_, ()| Ok(get_duration()))?;
    os.set("clock", os_clock)?;

    // os.time
    let os_time = state.create_function(|_, ()| {
        Ok(SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| Error::ExternalError(Arc::new(err)))?
            .as_secs())
    })?;
    os.set("time", os_time)?;

    state.globals().set("os", os)?;

    Ok(())
}
