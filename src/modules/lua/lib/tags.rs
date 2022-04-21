use anyhow::Result;
use crossbeam::channel::Sender;
use mlua::{prelude::*, Error as LuaError, Lua, MetaMethod, UserData, UserDataMethods};
use std::sync::Arc;

use super::{
    super::state::LuaAsyncCallback,
    bot::{BotServer, BotUser},
};
use crate::bot::{db::{Tag, Uid}, Bot};

#[derive(Debug, PartialEq)]
enum TagPart {
    Codeblock(String, String),
    Text(String),
    Tag(String, String),
    Var(String),
}

fn parse_tag(value: &str) -> Vec<TagPart> {
    let mut out = Vec::new();

    let mut text = String::new();
    let mut closures_deep = 0;
    let mut tag = None;

    let mut chars = value.chars();

    let mut offset = 0;
    while let Some(c) = chars.next() {
        let rest = &value[offset..];
        offset += c.len_utf8();

        if c == '{' {
            if !text.is_empty() && closures_deep == 0 {
                out.push(TagPart::Text(std::mem::replace(&mut text, String::new())));
            }

            closures_deep += 1;
        } else if c == '}' {
            if closures_deep > 0 {
                if closures_deep == 1 {
                    if let Some(tag) = tag.take() {
                        out.push(TagPart::Tag(
                            tag,
                            std::mem::replace(&mut text, String::new()),
                        ));
                        closures_deep -= 1;
                        continue;
                    }

                    let name = &text[1..];

                    if name.chars().all(|c| c.is_ascii_lowercase()) {
                        out.push(TagPart::Var(name.to_string()));
                        text.clear();
                        closures_deep -= 1;
                        continue;
                    }
                }

                closures_deep -= 1;
            }
        } else if c == ':' && closures_deep == 1 && !text.is_empty() {
            let name = &text[1..];

            if name.chars().all(|c| c.is_ascii_lowercase()) {
                tag = Some(name.to_string());
                text.clear();
                continue;
            }
        } else if c == '`' && closures_deep == 0 && rest.starts_with("```") {
            if !text.is_empty() && closures_deep == 0 {
                out.push(TagPart::Text(std::mem::replace(&mut text, String::new())));
            }

            let rest2 = &rest[3..];

            if let Some(end) = rest2.find("```") {
                let lang_size = rest2
                    .chars()
                    .take_while(|c| !c.is_whitespace())
                    .map(|c| c.len_utf8())
                    .sum();

                if lang_size + 1 <= end {
                    let content = &rest2[lang_size + 1..end];

                    if rest2.len() > lang_size && !content.is_empty() {
                        let lang = &rest2[..lang_size];

                        out.push(TagPart::Codeblock(lang.into(), content.into()));

                        for _ in 0..(end + 6) {
                            chars.next();
                        }

                        continue;
                    }
                }
            }
        }

        text.push(c);
    }

    if let Some(tag) = tag {
        if !text.is_empty() {
            out.push(TagPart::Text(text));
        } else {
            out.push(TagPart::Text([tag, text].concat()));
        }
    } else {
        if !text.is_empty() {
            out.push(TagPart::Text(text));
        }
    }

    out
}

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

    let parse_tag_fn = state.create_function(move |state, value: String| {
        let out = state.create_table()?;

        for (i, tag_part) in parse_tag(&value).into_iter().enumerate() {
            let i = i + 1;
            match tag_part {
                TagPart::Codeblock(lang, value) => {
                    let tbl = state.create_table()?;

                    tbl.set("codeblock", lang)?;
                    tbl.set("value", value)?;

                    out.raw_insert(i as i64, tbl)?
                }
                TagPart::Text(text) => out.raw_insert(i as i64, text)?,
                TagPart::Tag(tag, value) => {
                    let tbl = state.create_table()?;

                    tbl.set("tag", tag)?;
                    tbl.set("value", value)?;

                    out.raw_insert(i as i64, tbl)?
                }
                TagPart::Var(var) => {
                    let tbl = state.create_table()?;

                    tbl.set("var", var)?;

                    out.raw_insert(i as i64, tbl)?
                }
            }
        }

        Ok(out)
    })?;
    tags_tbl.set("parse_tag", parse_tag_fn)?;

    state.globals().set("tags", tags_tbl)?;

    Ok(())
}

pub struct LuaTag {
    bot: Arc<Bot>,
    sender: Sender<LuaAsyncCallback>,
    inner: Tag,
}

impl LuaTag {
    pub fn owner_uid(&self) -> Uid {
        self.inner.uid
    }

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

#[cfg(test)]
mod tests {
    use super::{parse_tag, TagPart};

    #[test]
    fn parse_tag_test() {
        let parts = parse_tag("text {a:b} aaa {test} b {c: a{}b} ```lua print(1)```");

        assert_eq!(
            parts,
            vec![
                TagPart::Text("text ".into()),
                TagPart::Tag("a".into(), "b".into()),
                TagPart::Text(" aaa ".into()),
                TagPart::Var("test".into()),
                TagPart::Text(" b ".into()),
                TagPart::Tag("c".into(), " a{}b".into()),
                TagPart::Text(" ".into()),
                TagPart::Codeblock("lua".into(), "print(1)".into()).into()
            ]
        )
    }
}
