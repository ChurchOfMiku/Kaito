use mlua::{UserData, Lua, AnyUserData, Function};
use crate::bot::{db::Uid, Bot};

#[derive(Default, Debug)]
pub struct TrustCtx {
	trusted: bool
}
impl TrustCtx {
	pub async fn update(mut self, bot: &Bot, uid: Uid) -> Self {
		if self.trusted { return self };
		
		if let Ok(user) = bot.db().get_user_from_uid(uid).await {
			self.trusted = crate::bot::is_trusted_role(&user.role);
		}

		self
	}
}
impl UserData for TrustCtx {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(_methods: &mut M) {}
}

pub trait Trust {
	fn is_in_trusted_context(&self) -> bool;
}
impl Trust for Lua {
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