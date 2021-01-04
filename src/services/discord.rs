use anyhow::Result;
use arc_swap::ArcSwapOption;
use futures::future::{AbortHandle, Abortable};
use parking_lot::Mutex;
use serenity::{
    model::{channel::Message, gateway::Ready},
    prelude::*,
    CacheAndHttp,
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
    cache_and_http: ArcSwapOption<CacheAndHttp>,
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
            | ServiceFeatures::VOICE.bits()
            | ServiceFeatures::MARKDOWN.bits(),
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
            cache_and_http: ArcSwapOption::new(None),
            ready_abort: Default::default(),
        });

        let client = Client::builder(&config.token)
            .event_handler(SerenityHandler::new(service.clone()))
            .await?;

        service
            .cache_and_http
            .store(Some(client.cache_and_http.clone()));

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

    async fn current_user(self: &Arc<DiscordService>) -> Result<Arc<user::DiscordUser>> {
        Ok(Arc::new(user::DiscordUser::new(
            self.cache_and_http().cache.current_user().await.into(),
            self.clone(),
        )))
    }
}

impl DiscordService {
    fn cache_and_http(&self) -> Arc<CacheAndHttp> {
        self.cache_and_http.load_full().unwrap()
    }
}

#[derive(Error, Debug)]
pub enum DiscordError {
    #[error("the channel does not have a guild")]
    NoChannelGuild,
    #[error("cache miss")]
    CacheMiss,
}
