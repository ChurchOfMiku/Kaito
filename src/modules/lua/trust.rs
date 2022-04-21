//! This module is basically one giant hack to allow `trusted` role users to do some things guests can't. For example, make HTTP POST requests.

use crate::bot::{
    db::{Uid, User},
    Bot,
};
use mlua::{AnyUserData, Function, Lua, UserData};

// TODO each trust ctx should have an ID that is invalidated when disposed
pub struct PendingTrustCtx {
    tag_owner: Option<Uid>,
    msg_user_trusted: bool,
}
impl PendingTrustCtx {
    pub fn new(msg_user: &User, tag_owner: Option<Uid>) -> Self {
        PendingTrustCtx {
            tag_owner,
            msg_user_trusted: crate::bot::is_trusted_role(&msg_user.role),
        }
    }

    pub async fn resolve(self, bot: &Bot) -> TrustCtx {
        TrustCtx {
            trusted: if let Some(tag_owner) = self.tag_owner {
                bot.db()
                    .get_user_from_uid(tag_owner)
                    .await
                    .map(|user| crate::bot::is_trusted_role(&user.role))
                    .unwrap_or(false)
            } else {
                self.msg_user_trusted
            },
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TrustCtx {
    pub trusted: bool,
}
impl UserData for TrustCtx {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(_methods: &mut M) {}
}

pub trait Trust {
    fn find_trust_context(&self) -> Option<TrustCtx>;
    fn is_in_trusted_context(&self) -> bool;
}
impl Trust for Lua {
    fn find_trust_context(&self) -> Option<TrustCtx> {
        let result = (|| {
            let trust_ctx = self.globals().get::<_, Function>("__TRUST_CTX_BUBBLE")?;
            let trust_ctx: TrustCtx = trust_ctx.call(())?;
            Ok::<_, mlua::Error>(trust_ctx)
        })();

        result.ok()
    }

    fn is_in_trusted_context(&self) -> bool {
        let result = (|| {
            let trust_ctx = self.globals().get::<_, Function>("__TRUST_CTX_BUBBLE")?;
            let trust_ctx: AnyUserData = trust_ctx.call(())?;
            let trust_ctx = trust_ctx.borrow::<TrustCtx>()?;
            Ok::<_, mlua::Error>(trust_ctx.trusted)
        })();

        result.unwrap_or(false)
    }
}
