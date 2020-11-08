use anyhow::Result;
use serenity::model::channel;
use std::sync::Arc;

use super::{channel::DiscordChannel, user::DiscordUser, DiscordService};
use crate::services::Message;

pub struct DiscordMessage {
    author: Arc<DiscordUser>,
    msg: channel::Message,
    service: Arc<DiscordService>,
}

impl DiscordMessage {
    pub fn new(msg: channel::Message, service: Arc<DiscordService>) -> DiscordMessage {
        let author = Arc::new(DiscordUser::new(msg.author.clone(), service.clone()));
        DiscordMessage {
            author,
            msg,
            service,
        }
    }
}

#[async_trait]
impl Message<DiscordService> for DiscordMessage {
    fn author(&self) -> &Arc<DiscordUser> {
        &self.author
    }

    fn content(&self) -> &str {
        &self.msg.content
    }

    async fn channel(&self) -> Result<Arc<DiscordChannel>> {
        // Check the cache
        if let Some(channel) = self.msg.channel(&self.service.cache).await {
            return Ok(Arc::new(DiscordChannel::new(channel, self.service.clone())));
        }

        // Fallback to REST
        let channel = self
            .service
            .http
            .get_channel(*self.msg.channel_id.as_u64())
            .await?;
        Ok(Arc::new(DiscordChannel::new(channel, self.service.clone())))
    }

    fn service(&self) -> &Arc<DiscordService> {
        &self.service
    }
}
