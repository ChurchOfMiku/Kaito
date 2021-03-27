use anyhow::Result;
use serenity::{builder::CreateEmbed, model::channel};
use std::sync::Arc;

use super::{channel::DiscordChannel, user::DiscordUser, DiscordService};
use crate::{
    message::{Attachment, MessageContent, MessageEmbed, MessageSettings, ToMessageContent},
    services::{Message, MessageId},
};

pub fn create_discord_embed(embed: MessageEmbed, mut e: &mut CreateEmbed) -> &mut CreateEmbed {
    // Set up the author
    if let Some(author_name) = embed.author_name {
        let author_icon_url = embed.author_icon_url;
        let author_url = embed.author_url;
        e = e.author(move |mut a| {
            a = a.name(author_name);

            if let Some(author_icon_url) = author_icon_url {
                a = a.icon_url(author_icon_url);
            }

            if let Some(author_url) = author_url {
                a = a.url(author_url);
            }

            a
        });
    }

    if let Some(color) = embed.color {
        e = e.color(color);
    }

    if let Some(description) = embed.description {
        e = e.description(description);
    }

    for (name, value, inline) in embed.fields {
        e = e.field(name, value, inline);
    }

    // Build the footer
    if let (Some(text), Some(icon_url)) = (embed.footer_text.clone(), embed.footer_icon_url.clone())
    {
        e = e.footer(move |f| f.icon_url(icon_url).text(text));
    } else if let Some(text) = embed.footer_text {
        e = e.footer(move |f| f.text(text));
    } else if let Some(icon_url) = embed.footer_icon_url {
        e = e.footer(move |f| f.icon_url(icon_url));
    }

    if let Some(image) = embed.image {
        e = e.image(image);
    }

    if let Some(thumbnail) = embed.thumbnail {
        e = e.thumbnail(thumbnail);
    }

    if let Some(timestamp) = embed.timestamp {
        e = e.timestamp(timestamp.to_rfc3339());
    }

    if let Some(title) = embed.title {
        e = e.title(title);
    }

    if let Some(attachment) = embed.attachment {
        e = e.attachment(attachment);
    }

    e
}

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

    async fn edit<'a, C>(&self, content: C, settings: MessageSettings) -> Result<()>
    where
        C: ToMessageContent<'a>,
    {
        let content = match content.to_message_content() {
            MessageContent::String(text) => text,
            MessageContent::Str(text) => text.to_string(),
        };

        self.channel()
            .await?
            .inner()
            .id()
            .edit_message(&self.service.cache_and_http().http, self.msg.id, |mut m| {
                if !content.is_empty() {
                    m = m.content(content);
                }

                if let Some(embed) = settings.embed {
                    m = m.embed(|e| create_discord_embed(embed, e));
                }

                m
            })
            .await?;

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
