use std::sync::Arc;

use super::{channel::DiscordChannel, user::DiscordUser, DiscordService};
use crate::services::Message;

pub struct DiscordMessage {}

impl Message<DiscordService> for DiscordMessage {
    fn author(&self) -> &Arc<DiscordUser> {
        unimplemented!();
    }

    fn channel(&self) -> &Arc<DiscordChannel> {
        unimplemented!();
    }

    fn service(&self) -> &Arc<DiscordService> {
        unimplemented!();
    }
}
