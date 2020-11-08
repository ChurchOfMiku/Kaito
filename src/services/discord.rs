use anyhow::Result;
use std::sync::Arc;

mod channel;
mod message;
mod server;
mod user;

use super::{Service, ServiceKind};

pub struct DiscordService {}

pub struct DiscordServiceConfig {
    token: String,
}

#[async_trait]
impl Service for DiscordService {
    const KIND: ServiceKind = ServiceKind::Discord;
    const NAME: &'static str = "Discord";

    type ServiceConfig = DiscordServiceConfig;
    type Message = message::DiscordMessage;
    type User = user::DiscordUser;
    type Channel = channel::DiscordChannel;
    type Server = server::DiscordServer;

    async fn init(config: Self::ServiceConfig) -> Result<Arc<Self>> {
        unimplemented!();
    }
    async fn unload(&self) -> Result<()> {
        Ok(())
    }
}
