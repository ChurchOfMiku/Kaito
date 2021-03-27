use anyhow::Result;
use serenity::model::channel;
use std::sync::Arc;

use super::{channel::DiscordChannel, user::DiscordUser, DiscordService};
use crate::{
    message::{Attachment, MessageContent, ToMessageContent},
    services::{Message, MessageId},
};

pub struct DiscordMessage {
    author: Arc<DiscordUser>,
    msg: channel::Message,
    service: Arc<DiscordService>,
    attachments: Vec<Arc<Attachment>>,
}

impl DiscordMessage {
    pub fn new(msg: channel::Message, service: Arc<DiscordService>) -> DiscordMessage {
        let attachments = msg
            .attachments
            .iter()
            .map(|a| {
                Arc::new(Attachment {
                    filename: a.filename.to_string(),
                    url: a.proxy_url.to_string(),
                    size: Some(a.size),
                    dimensions: a.dimensions(),
                })
            })
            .collect();

        let author = Arc::new(DiscordUser::new(msg.author.clone(), service.clone()));
        DiscordMessage {
            author,
            msg,
            service,
            attachments,
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
        let cache_and_http = self.service().cache_and_http();
        // Check the cache
        if let Some(channel) = self.msg.channel(&cache_and_http.cache).await {
            return Ok(Arc::new(DiscordChannel::new(channel, self.service.clone())));
        }

        // Fallback to REST
        let channel = cache_and_http
            .http
            .get_channel(*self.msg.channel_id.as_u64())
            .await?;
        Ok(Arc::new(DiscordChannel::new(channel, self.service.clone())))
    }

    async fn edit<'a, C>(&self, content: C) -> Result<()>
    where
        C: ToMessageContent<'a>,
    {
        match content.to_message_content() {
            MessageContent::String(text) => {
                self.channel()
                    .await?
                    .inner()
                    .id()
                    .edit_message(&self.service.cache_and_http().http, self.msg.id, |m| {
                        m.content(text)
                    })
                    .await?
            }
            MessageContent::Str(text) => {
                self.channel()
                    .await?
                    .inner()
                    .id()
                    .edit_message(&self.service.cache_and_http().http, self.msg.id, |m| {
                        m.content(text)
                    })
                    .await?
            }
        };

        Ok(())
    }

    async fn delete(&self) -> Result<()> {
        self.msg.delete(&self.service.cache_and_http()).await?;

        Ok(())
    }

    fn attachments(&self) -> &[Arc<Attachment>] {
        &self.attachments
    }

    fn service(&self) -> &Arc<DiscordService> {
        &self.service
    }

    fn id(&self) -> MessageId {
        MessageId::Discord(*self.msg.id.as_u64())
    }
}
