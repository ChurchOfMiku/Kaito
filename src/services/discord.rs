use anyhow::Result;
use futures::future::{AbortHandle, Abortable};
use parking_lot::Mutex;
use serenity::{
    cache::Cache,
    http::client::Http,
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use std::sync::Arc;
use thiserror::Error;

mod channel;
mod message;
mod server;
mod user;

use super::{Service, ServiceFeatures, ServiceKind};
use crate::bot::Bot;

pub struct DiscordService {
    bot: Arc<Bot>,
    cache: Arc<Cache>,
    http: Arc<Http>,
    ready_abort: Mutex<Option<AbortHandle>>,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct DiscordServiceConfig {
    pub token: String,
}

struct SerenityHandler {
    service: Arc<DiscordService>,
}

impl SerenityHandler {
    pub fn new(service: Arc<DiscordService>) -> Self {
        SerenityHandler { service }
    }
}

#[async_trait]
impl EventHandler for SerenityHandler {
    async fn ready(&self, _: Context, ready: Ready) {
        if let Some(abort_handle) = self.service.ready_abort.lock().take() {
            abort_handle.abort();
        }

        println!(
            "{}#{} is connected!",
            ready.user.name, ready.user.discriminator
        );
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        let msg = message::DiscordMessage::new(msg, self.service.clone());
        self.service.bot.message(Arc::new(msg)).await;
    }
}

#[async_trait]
impl Service for DiscordService {
    const KIND: ServiceKind = ServiceKind::Discord;
    const NAME: &'static str = "Discord";
    const FEATURES: ServiceFeatures = ServiceFeatures::from_bits_truncate(
        ServiceFeatures::EMBEDS.bits()
            | ServiceFeatures::REACTIONS.bits()
            | ServiceFeatures::VOICE.bits(),
    );

    type ServiceConfig = DiscordServiceConfig;
    type Message = message::DiscordMessage;
    type User = user::DiscordUser;
    type Channel = channel::DiscordChannel;
    type Server = server::DiscordServer;

    type ChannelId = u64;
    type ServerId = u64;
    type UserId = u64;

    async fn init(bot: Arc<Bot>, config: Self::ServiceConfig) -> Result<Arc<Self>> {
        let service = Arc::new(DiscordService {
            bot,
            cache: unsafe { Arc::from_raw(std::ptr::null()) },
            http: unsafe { Arc::from_raw(std::ptr::null()) },
            ready_abort: Default::default(),
        });

        let client = Client::builder(&config.token)
            .event_handler(SerenityHandler::new(service.clone()))
            .await?;

        // No events is run before the client is started, therefore this is safe?
        unsafe {
            std::ptr::write_unaligned(
                &service.cache as *const _ as *mut _,
                client.cache_and_http.cache.clone(),
            );
            std::ptr::write_unaligned(
                &service.http as *const _ as *mut _,
                client.cache_and_http.http.clone(),
            );
        }

        async fn wrap_client(mut client: Client) -> Result<()> {
            client.start().await?;

            Ok(())
        }

        let join_task = tokio::spawn(wrap_client(client));

        // Block on the client task until it is ready or it has errored and yielded
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let join_task = Abortable::new(join_task, abort_registration);
        *service.ready_abort.lock() = Some(abort_handle);

        if let Ok(res) = join_task.await {
            res??;
        }

        Ok(service)
    }
    async fn unload(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum DiscordError {
    #[error("the channel does not have a guild")]
    NoChannelGuild,
    #[error("cache miss")]
    CacheMiss,
}
