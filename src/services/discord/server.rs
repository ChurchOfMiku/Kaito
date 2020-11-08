use std::sync::Arc;

use super::DiscordService;
use crate::services::Server;

pub struct DiscordServer {}

impl Server<DiscordService> for DiscordServer {
    fn service(&self) -> &Arc<DiscordService> {
        unimplemented!();
    }
}
