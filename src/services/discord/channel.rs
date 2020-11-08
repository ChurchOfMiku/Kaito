use std::sync::Arc;

use super::{server::DiscordServer, DiscordService};
use crate::services::Channel;

pub struct DiscordChannel {}

impl Channel<DiscordService> for DiscordChannel {
    fn server(&self) -> &Arc<DiscordServer> {
        unimplemented!();
    }

    fn service(&self) -> &Arc<DiscordService> {
        unimplemented!();
    }
}
