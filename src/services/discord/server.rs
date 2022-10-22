use serenity::model::guild;
use std::sync::Arc;

use super::DiscordService;
use crate::services::{Server, ServerId};

pub struct DiscordServer {
    guild: guild::Guild,
    service: Arc<DiscordService>,
}

impl DiscordServer {
    pub fn new(guild: guild::Guild, service: Arc<DiscordService>) -> DiscordServer {
        DiscordServer { guild, service }
    }
}

#[async_trait]
impl Server<DiscordService> for DiscordServer {
    fn id(&self) -> ServerId {
        ServerId::Discord(self.guild.id.0)
    }

    fn name(&self) -> &str {
        &self.guild.name
    }

    fn service(&self) -> &Arc<DiscordService> {
        &self.service
    }
}

impl DiscordServer {
    pub fn guild(&self) -> &guild::Guild {
        &self.guild
    }
}
