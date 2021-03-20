use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{
    prelude::{LuaError, LuaMultiValue},
    Function, Lua, RegistryKey, Table,
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;

use super::super::state::LuaAsyncCallback;

pub fn create_future(state: &Lua) -> Result<(RegistryKey, Table)> {
    let async_tbl: Table = state.globals().get("async")?;
    let fut_fn: Function = async_tbl.get("__RustFuture")?;

    let fut: Table = fut_fn.call(())?;
    let fut_reg_key = state.create_registry_value(fut.clone())?;

    Ok((fut_reg_key, fut))
}

macro_rules! create_lua_future {
    ($state:expr, $sender:expr, $data:expr, $fut:expr, |$state_ident:ident, $data_ident:ident: $data_ty:ty, $res:ident: $res_ty:ty| $closure:block) => {{
        use mlua::ToLuaMulti;

        let (future_reg_key, fut) = match $crate::modules::lua::lib::r#async::create_future($state)
        {
            Ok(a) => a,
            Err(err) => {
                return Err(LuaError::ExternalError(Arc::new(
                    $crate::modules::lua::lib::r#async::AsyncError::FutureError(err.to_string()),
                )).into())
            }
        };

        let sandbox_state = $state.named_registry_value("__SANDBOX_STATE").ok().clone();

        let sender = $sender.clone();
        let data = $data;
        tokio::spawn(async move {
            let fut_res = $fut.await;

            let callback: Box<dyn for<'c> FnOnce(&'c Lua) -> anyhow::Result<LuaMultiValue<'c>> + Send> = Box::new(move |state| {
                fn lua_callback<'a>($state_ident: &'a Lua, $data_ident: $data_ty, $res: $res_ty) -> anyhow::Result<impl ToLuaMulti<'a>> $closure

                match lua_callback(state, data, fut_res) {
                    Ok(data) => Ok(data.to_lua_multi(state)?),
                    Err(err) => Err(err),
                }
            });

            sender
                .send((
                    future_reg_key,
                    sandbox_state,
                    callback,
                ))
                .unwrap();
        });

        fut
    }};
}

pub fn lib_async(
    state: &Lua,
    sender: Sender<LuaAsyncCallback>,
    thread_id: Arc<AtomicU64>,
) -> Result<()> {
    let async_tbl = state.create_table()?;

    // async.delay
    let async_delay = state.create_function(move |state, duration: f64| {
        if duration.is_sign_negative() || !duration.is_finite() {
            return Err(LuaError::ExternalError(Arc::new(
                AsyncError::InvalidDuration,
            )));
        }

        let duration = Duration::from_secs_f64(duration);

        let fut = create_lua_future!(
            state,
            sender,
            (),
            tokio::time::sleep(duration),
            |_state, _data: (), _res: ()| { Ok(()) }
        );

        Ok(fut)
    })?;
    async_tbl.set("delay", async_delay)?;

    let gen_thread_id_fn = state
        .create_function(move |_state, (): ()| Ok(thread_id.fetch_add(1, Ordering::Relaxed)))?;
    async_tbl.set("gen_thread_id", gen_thread_id_fn)?;

    state.globals().set("async", async_tbl)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum AsyncError {
    #[error("invalid duration")]
    InvalidDuration,
    #[error("{}", _0)]
    FutureError(String),
}
