use anyhow::Result;
use serenity::model::guild;
use std::sync::Arc;

use super::DiscordService;
use crate::services::{ChannelId, Server, ServerId, Service, User, UserId};

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

    async fn voice_user_channel(&self, user: u64) -> Result<Option<ChannelId>> {
        match self
            .guild
            .voice_states
            .get(&serenity::model::id::UserId::from(user))
            .and_then(|s| s.channel_id)
        {
            Some(id) => Ok(Some(ChannelId::Discord(*id.as_u64()))),
            None => Ok(None),
        }
    }

    async fn voice_channel_users(&self, channel_id: u64) -> Result<Vec<UserId>> {
        let mut ids = Vec::new();

        let bot_id = self.service.current_user().await?.id();

        for (user_id, voice_state) in &self.guild.voice_states {
            if voice_state
                .channel_id
                .map(|id| id.as_u64() == &channel_id)
                .unwrap_or(false)
            {
                let id = UserId::Discord(*user_id.as_u64());

                if bot_id != id {
                    ids.push(id);
                }
            }
        }

        Ok(ids)
    }
}

impl DiscordServer {
    pub fn guild(&self) -> &guild::Guild {
        &self.guild
    }
}
