use serenity::model::user;
use std::sync::Arc;

use super::DiscordService;
use crate::services::{User, UserId};

pub struct DiscordUser {
    user: user::User,
    service: Arc<DiscordService>,
    name: String,
    nick: String,
}

impl DiscordUser {
    pub fn new(user: user::User, service: Arc<DiscordService>) -> DiscordUser {
        DiscordUser {
            name: format!("{}#{}", user.name, user.discriminator),
            nick: user.name.clone(),
            user,
            service,
        }
    }
}

impl User<DiscordService> for DiscordUser {
    fn id(&self) -> UserId {
        UserId::Discord(self.user.id.0)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn nick(&self) -> &str {
        &self.nick
    }

    fn avatar(&self) -> &Option<String> {
        &self.user.avatar
    }

    fn bot(&self) -> Option<bool> {
        Some(self.user.bot)
    }

    fn service(&self) -> &Arc<DiscordService> {
        &self.service
    }
}
