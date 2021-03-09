use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{prelude::*, Error as LuaError, Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;

use super::{
    super::state::LuaAsyncCallback,
    bot::{BotServer, BotUser},
};
use crate::bot::{db::Tag, Bot};

pub fn lib_tags(state: &Lua, bot: &Arc<Bot>, sender: Sender<LuaAsyncCallback>) -> Result<()> {
    let tags_tbl = state.create_table()?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let find_tag_fn =
        state.create_function(move |state, (server, tag_key): (LuaAnyUserData, String)| {
            let bot = bot2.clone();
            let server = server.borrow::<BotServer>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2.clone(),
                (bot.clone(), sender2.clone()),
                bot.db().find_tag(server.id(), &tag_key.to_lowercase()),
                |state, data: (Arc<Bot>, Sender<LuaAsyncCallback>), res: Result<Option<Tag>>| {
                    match res? {
                        Some(tag) => Ok(LuaValue::UserData(
                            state.create_userdata(LuaTag::from_tag(data.0, data.1, tag)?)?,
                        )),
                        None => Ok(LuaValue::Nil),
                    }
                }
            );

            Ok(fut)
        })?;
    tags_tbl.set("find_tag", find_tag_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let create_tag_fn = state.create_function(
        move |state,
              (user, server, tag_key, tag_value): (
            LuaAnyUserData,
            LuaAnyUserData,
            String,
            String,
        )| {
            let bot = bot2.clone();
            let user = user.borrow::<BotUser>()?.clone();
            let server = server.borrow::<BotServer>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                bot.db()
                    .create_tag(user.uid(), server.id(), &tag_key.to_lowercase(), &tag_value),
                |state, _data: (), res: Result<bool>| {
                    if res? {
                        Ok(LuaValue::Nil)
                    } else {
                        Ok(LuaValue::String(state.create_string("tag already exists")?))
                    }
                }
            );

            Ok(fut)
        },
    )?;
    tags_tbl.set("create_tag", create_tag_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let count_user_tags_fn = state.create_function(move |state, user: LuaAnyUserData| {
        let bot = bot2.clone();
        let user = user.borrow::<BotUser>()?.clone();

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            bot.db().count_uid_tags(user.uid()),
            |_state, _data: (), res: Result<i64>| { Ok(res?) }
        );

        Ok(fut)
    })?;
    tags_tbl.set("count_user_tags", count_user_tags_fn)?;

    let bot2 = bot.clone();
    let sender2 = sender.clone();
    let list_tags_fn = state.create_function(
        move |state, (user, server): (LuaAnyUserData, LuaAnyUserData)| {
            let bot = bot2.clone();
            let user = user.borrow::<BotUser>()?.clone();
            let server = server.borrow::<BotServer>()?.clone();

            let fut = create_lua_future!(
                state,
                sender2,
                (),
                bot.db().list_tags(user.uid(), server.id()),
                |_state, _data: (), res: Result<Vec<String>>| { res }
            );

            Ok(fut)
        },
    )?;
    tags_tbl.set("list_tags", list_tags_fn)?;

    state.globals().set("tags", tags_tbl)?;

    Ok(())
}
pub struct LuaTag {
    bot: Arc<Bot>,
    sender: Sender<LuaAsyncCallback>,
    inner: Tag,
}

impl LuaTag {
    pub fn from_tag(bot: Arc<Bot>, sender: Sender<LuaAsyncCallback>, inner: Tag) -> Result<LuaTag> {
        Ok(LuaTag { bot, sender, inner })
    }
}

impl UserData for LuaTag {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        methods.add_method("edit", |state, tag, value: String| {
            let (bot, sid, key) = (tag.bot.clone(), tag.inner.sid, tag.inner.key.clone());

            let fut = create_lua_future!(
                state,
                tag.sender,
                (),
                bot.db().edit_tag(sid, &key, &value),
                |_state, _data: (), res: Result<()>| {
                    res?;

                    Ok(())
                }
            );

            Ok(fut)
        });

        methods.add_method("delete", |state, tag, _: ()| {
            let (bot, sid, key) = (tag.bot.clone(), tag.inner.sid, tag.inner.key.clone());

            let fut = create_lua_future!(
                state,
                tag.sender,
                (),
                bot.db().delete_tag(sid, &key),
                |_state, _data: (), res: Result<()>| {
                    res?;

                    Ok(())
                }
            );

            Ok(fut)
        });

        methods.add_method("set_owner", |state, tag, user: LuaAnyUserData| {
            let (bot, sid, key) = (tag.bot.clone(), tag.inner.sid, tag.inner.key.clone());

            let uid = user.borrow::<BotUser>()?.uid();

            let fut = create_lua_future!(
                state,
                tag.sender,
                (),
                bot.db().set_tag_uid(sid, &key, uid),
                |_state, _data: (), res: Result<()>| {
                    res?;

                    Ok(())
                }
            );

            Ok(fut)
        });

        methods.add_method(
            "set_transfer_user",
            |state, tag, user: Option<LuaAnyUserData>| {
                let (bot, sid, key) = (tag.bot.clone(), tag.inner.sid, tag.inner.key.clone());

                let uid = match user {
                    Some(data) => Some(data.borrow::<BotUser>()?.uid()),
                    None => None,
                };

                let fut = create_lua_future!(
                    state,
                    tag.sender,
                    (),
                    bot.db().set_tag_transfer_uid(sid, &key, uid),
                    |_state, _data: (), res: Result<()>| {
                        res?;

                        Ok(())
                    }
                );

                Ok(fut)
            },
        );

        methods.add_meta_method(MetaMethod::Index, |state, tag, index: String| {
            match index.as_str() {
                "uid" => Ok(mlua::Value::Number(tag.inner.uid as _)),
                "transfer_uid" => Ok(match tag.inner.transfer_uid {
                    Some(uid) => mlua::Value::Number(uid as f64),
                    None => mlua::Value::Nil,
                }),
                "value" => Ok(mlua::Value::String(state.create_string(&tag.inner.value)?)),
                _ => Ok(mlua::Value::Nil),
            }
        });
    }
}
