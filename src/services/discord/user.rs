use std::sync::Arc;

use super::DiscordService;
use crate::services::User;

pub struct DiscordUser {}

impl User<DiscordService> for DiscordUser {
    fn service(&self) -> &Arc<DiscordService> {
        unimplemented!();
    }
}
