use anyhow::Result;
use serenity::model::channel;
use std::{convert::TryInto, sync::Arc};

use super::{message::DiscordMessage, server::DiscordServer, DiscordError, DiscordService};
use crate::{
    message::{MessageContent, MessageSettings, ToMessageContent},
    services::{Channel, ChannelId},
};

pub struct DiscordChannel {
    channel: channel::Channel,
    service: Arc<DiscordService>,
}

impl DiscordChannel {
    pub fn new(channel: channel::Channel, service: Arc<DiscordService>) -> DiscordChannel {
        DiscordChannel { channel, service }
    }

    pub fn inner(&self) -> &channel::Channel {
        &self.channel
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

    async fn send<'a, C>(
        &self,
        content: C,
        settings: MessageSettings,
    ) -> Result<Arc<DiscordMessage>>
    where
        C: ToMessageContent<'a>,
    {
        let msg = match content.to_message_content() {
            MessageContent::String(text) => {
                self.channel
                    .id()
                    .send_message(&self.service.cache_and_http().http, |m| {
                        m.content(text).allowed_mentions(|am| {
                            am.empty_parse();

                            if let Some(mention_user) = settings.reply_user {
                                let a: Result<u64, _> = mention_user.try_into();
                                if let Ok(id) = a {
                                    am.users(vec![id]);
                                }
                            }

                            am
                        })
                    })
                    .await?
            }
            MessageContent::Str(text) => {
                self.channel
                    .id()
                    .send_message(&self.service.cache_and_http().http, |m| m.content(text))
                    .await?
            }
        };

        Ok(Arc::new(DiscordMessage::new(msg, self.service.clone())))
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

    async fn send_typing(&self) -> Result<()> {
        self.service()
            .cache_and_http()
            .http
            .broadcast_typing(*self.channel.id().as_u64())
            .await?;

        Ok(())
    }

    fn service(&self) -> &Arc<DiscordService> {
        &self.service
    }
}
