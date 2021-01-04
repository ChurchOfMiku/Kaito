use anyhow::Result;
use serenity::model::channel;
use std::sync::Arc;

use super::{server::DiscordServer, DiscordError, DiscordService};
use crate::services::{Channel, ChannelId};

pub struct DiscordChannel {
    channel: channel::Channel,
    service: Arc<DiscordService>,
}

impl DiscordChannel {
    pub fn new(channel: channel::Channel, service: Arc<DiscordService>) -> DiscordChannel {
        DiscordChannel { channel, service }
    }
}

#[async_trait]
impl Channel<DiscordService> for DiscordChannel {
    fn id(&self) -> ChannelId {
        ChannelId::Discord(self.channel.id().0)
    }

    fn name(&self) -> String {
        match &self.channel {
            channel::Channel::Guild(c) => c.name.clone(),
            channel::Channel::Private(c) => c.name(),
            channel::Channel::Category(c) => c.name.clone(),
            _ => unimplemented!("discord channel name"),
        }
    }

    async fn server(&self) -> Result<Arc<DiscordServer>> {
        let guild_id = match &self.channel {
            channel::Channel::Guild(c) => c.guild_id,
            channel::Channel::Category(c) => c.guild_id,
            _ => return Err(DiscordError::NoChannelGuild.into()),
        };

        let cache_and_http = self.service().cache_and_http();
        if let Some(guild) = cache_and_http.cache.guild(guild_id).await {
            return Ok(Arc::new(DiscordServer::new(guild, self.service.clone())));
        }

        Err(DiscordError::CacheMiss.into())
    }

    fn service(&self) -> &Arc<DiscordService> {
        &self.service
    }
}
