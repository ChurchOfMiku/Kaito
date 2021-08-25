use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{prelude::*, Lua, MetaMethod, UserData, UserDataMethods};
use std::{process::Output, sync::Arc, time::Duration};

use crate::{
    bot::Bot,
    modules::lua::state::LuaAsyncCallback,
    services::{ChannelId, ServerId, UserId, VoiceConnectionAbstract},
};

use super::bot::{BotServer, BotUser};

pub fn lib_voice(state: &Lua, bot: &Arc<Bot>, sender: Sender<LuaAsyncCallback>) -> Result<()> {
    let voice = state.create_table()?;

    // voice.join
    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let voice_join_fn =
        state.create_function(move |state, (server_id, channel_id): (String, String)| {
            let ctx = bot2.get_ctx();

            let server_id = ServerId::from_str(&server_id)
                .map_err(|err| LuaError::RuntimeError(err.to_string()))?;

            let channel_id = ChannelId::from_str(&channel_id)
                .map_err(|err| LuaError::RuntimeError(err.to_string()))?;

            let bot3 = bot2.clone();
            let sender3 = sender2.clone();
            let fut = create_lua_future!(
                state,
                sender2,
                (),
                async move {
                    let connection = ctx.services().join_voice(server_id, channel_id).await?;

                    Ok(LuaVoiceConnection(Arc::new(connection), bot3, sender3))
                },
                |_state, _data: (), res: Result<LuaVoiceConnection>| { Ok(res?) }
            );

            Ok(fut)
        })?;
    voice.set("join", voice_join_fn)?;

    // voice.user_channel
    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let user_channel_fn =
        state.create_function(move |state, (server, user): (BotServer, BotUser)| {
            let ctx = bot2.get_ctx();

            let server_id = server.id();
            let user_id = user.id();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                ctx.services().voice_user_channel(server_id, user_id),
                |_state, _data: (), res: Result<Option<ChannelId>>| {
                    Ok(res?.map(|id| id.to_short_str()))
                }
            );

            Ok(fut)
        })?;
    voice.set("user_channel", user_channel_fn)?;

    // voice.ytdl_metadata
    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let ytdl_metadata_fn = state.create_function(move |state, link: String| {
        let ctx = bot2.get_ctx();

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            tokio::process::Command::new("youtube-dl")
                .arg("-jq")
                .arg("--no-warnings")
                .arg(&link)
                .output(),
            |_state, _data: (), res: Result<Output, std::io::Error>| {
                Ok(res.map(|out| String::from_utf8_lossy(&out.stdout).to_string())?)
            }
        );

        Ok(fut)
    })?;
    voice.set("ytdl_metadata", ytdl_metadata_fn)?;

    state.globals().set("voice", voice)?;

    Ok(())
}

#[derive(Clone)]
struct LuaVoiceConnection(
    Arc<dyn VoiceConnectionAbstract>,
    Arc<Bot>,
    Sender<LuaAsyncCallback>,
);

impl UserData for LuaVoiceConnection {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method(
            "play",
            |state, conn, (input, seek): (String, Option<f64>)| {
                let voice_conn = conn.0.clone();

                let seek = seek
                    .filter(|seek| seek > &0.0)
                    .map(|seek| Duration::from_secs_f64(seek));

                let fut = create_lua_future!(
                    state,
                    conn.2,
                    (),
                    voice_conn.play(&input, seek),
                    |_state, _data: (), res: Result<()>| { Ok(res?) }
                );

                Ok(fut)
            },
        );

        methods.add_method("set_volume", |state, conn, volume: f32| {
            let voice_conn = conn.0.clone();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                voice_conn.set_volume(volume),
                |_state, _data: (), _res: ()| { Ok(()) }
            );

            Ok(fut)
        });

        methods.add_method("stop", |state, conn, (): ()| {
            let voice_conn = conn.0.clone();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                voice_conn.stop(),
                |_state, _data: (), res: Result<()>| { Ok(res?) }
            );

            Ok(fut)
        });

        methods.add_method("position", |state, conn, (): ()| {
            let voice_conn = conn.0.clone();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                voice_conn.position(),
                |_state, _data: (), res: Option<Duration>| { Ok(res.map(|d| d.as_secs_f64())) }
            );

            Ok(fut)
        });

        methods.add_method("connected", |state, conn, (): ()| {
            let voice_conn = conn.0.clone();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                voice_conn.connected(),
                |_state, _data: (), res: bool| { Ok(res) }
            );

            Ok(fut)
        });

        methods.add_method("disconnect", |state, conn, (): ()| {
            let voice_conn = conn.0.clone();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                voice_conn.disconnect(),
                |_state, _data: (), res: Result<()>| { Ok(res?) }
            );

            Ok(fut)
        });

        methods.add_method("playing", |state, conn, (): ()| {
            let voice_conn = conn.0.clone();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                voice_conn.playing(),
                |_state, _data: (), res: bool| { Ok(res) }
            );

            Ok(fut)
        });

        methods.add_method("listeners", |state, conn, (): ()| {
            let ctx = conn.1.clone().get_ctx();

            let server_id = conn.0.server_id();
            let channel_id = conn.0.channel_id();

            let fut = create_lua_future!(
                state,
                conn.2,
                (),
                ctx.services().voice_channel_users(server_id, channel_id),
                |state, _data: (), res: Result<Vec<UserId>>| {
                    let tbl = state.create_table()?;

                    for (idx, id) in res?.into_iter().enumerate() {
                        tbl.raw_insert((idx + 1) as i64, id.to_short_str())?;
                    }

                    Ok(tbl)
                }
            );

            Ok(fut)
        });

        methods.add_meta_method(
            MetaMethod::Index,
            |state, conn, index: String| match index.as_str() {
                "channel_id" => Ok(mlua::Value::String(
                    state.create_string(&conn.0.channel_id().to_short_str())?,
                )),
                "server_id" => Ok(mlua::Value::String(
                    state.create_string(&conn.0.server_id().to_short_str())?,
                )),
                _ => Ok(mlua::Value::Nil),
            },
        );

        methods.add_meta_method(MetaMethod::ToString, |state, conn, (): ()| {
            state.create_string(&format!(
                "VoiceConnection {{ channel_id = \"{}\" }}",
                conn.0.channel_id().to_str(),
            ))
        });
    }
}
