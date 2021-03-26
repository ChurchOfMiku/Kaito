use anyhow::{anyhow, Result};
use arc_swap::ArcSwapOption;
use async_mutex::Mutex as AsyncMutex;
use futures::future::{AbortHandle, Abortable};
use lru::LruCache;
use serenity::{
    http::CacheHttp,
    model::{
        channel::{Message, Reaction, ReactionType},
        event::MessageUpdateEvent,
        gateway::Ready,
    },
    prelude::*,
    CacheAndHttp,
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};
use thiserror::Error;

mod channel;
mod message;
mod server;
mod user;

use self::user::DiscordUser;

use super::{Channel, Service, ServiceFeatures, ServiceKind};
use crate::bot::Bot;

pub struct DiscordService {
    bot: Arc<Bot>,
    cache_and_http: ArcSwapOption<CacheAndHttp>,
    ready_abort: Mutex<Option<AbortHandle>>,
    user_cache: AsyncMutex<LruCache<u64, Arc<DiscordUser>>>,
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

    async fn reaction(&self, reaction: Reaction, remove: bool) {
        let user_id = match reaction.user_id {
            Some(id) => id,
            None => return,
        };

        let reactor = match self.service.user(*user_id.as_u64()).await {
            Ok(user) => user,
            Err(_) => return,
        };

        let msg = match self
            .service
            .message(*reaction.channel_id.as_u64(), *reaction.message_id.as_u64())
            .await
        {
            Ok(message) => message,
            Err(_) => return,
        };

        self.service
            .bot
            .reaction(msg, reactor, reaction.emoji.as_data(), remove)
            .await
    }
}

#[async_trait]
impl EventHandler for SerenityHandler {
    async fn ready(&self, _: Context, ready: Ready) {
        if let Some(abort_handle) = self.service.ready_abort.lock().unwrap().take() {
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

    async fn message_update(
        &self,
        _ctx: Context,
        old: Option<Message>,
        new: Option<Message>,
        _event: MessageUpdateEvent,
    ) {
        if let Some(new) = new {
            let msg = Arc::new(message::DiscordMessage::new(new, self.service.clone()));
            let old_msg = old.map(|msg| {
                Arc::new(message::DiscordMessage::new(msg, self.service.clone())) as Arc<_>
            });
            self.service.bot.message_update(msg, old_msg).await;
        }
    }

    async fn reaction_add(&self, _ctx: Context, reaction: Reaction) {
        self.reaction(reaction, false).await;
    }

    async fn reaction_remove(&self, _ctx: Context, reaction: Reaction) {
        self.reaction(reaction, true).await;
    }
}

#[async_trait]
impl Service for DiscordService {
    const KIND: ServiceKind = ServiceKind::Discord;
    const ID: &'static str = "discord";
    const ID_SHORT: &'static str = "d";
    const NAME: &'static str = "Discord";
    const FEATURES: ServiceFeatures = ServiceFeatures::from_bits_truncate(
        ServiceFeatures::EDIT.bits()
            | ServiceFeatures::EMBED.bits()
            | ServiceFeatures::REACT.bits()
            | ServiceFeatures::VOICE.bits()
            | ServiceFeatures::MARKDOWN.bits(),
    );

    type ServiceConfig = DiscordServiceConfig;
    type Message = message::DiscordMessage;
    type User = user::DiscordUser;
    type Channel = channel::DiscordChannel;
    type Server = server::DiscordServer;

    type MessageId = u64;
    type ChannelId = u64;
    type ServerId = u64;
    type UserId = u64;

    async fn init(bot: Arc<Bot>, config: Self::ServiceConfig) -> Result<Arc<Self>> {
        let service = Arc::new(DiscordService {
            bot,
            cache_and_http: ArcSwapOption::new(None),
            ready_abort: Default::default(),
            user_cache: AsyncMutex::new(LruCache::new(64)),
        });

        let client;
        let mut retry_count = 1;

        loop {
            match Client::builder(&config.token)
                .event_handler(SerenityHandler::new(service.clone()))
                .await
            {
                Ok(c) => break client = c,
                Err(err) => {
                    let time = 2 ^ retry_count;
                    retry_count += 1;
                    println!(
                        "Error creating discord client: {}, retrying in {} seconds",
                        err.to_string(),
                        time
                    );

                    tokio::time::sleep(std::time::Duration::from_secs(time)).await;
                }
            }
        }

        client
            .cache_and_http
            .cache()
            .as_ref()
            .unwrap()
            .set_max_messages(64)
            .await;

        service
            .cache_and_http
            .store(Some(client.cache_and_http.clone()));

        async fn wrap_client(mut client: Client) -> Result<()> {
            let mut retry_count = 1;

            loop {
                match client.start().await {
                    Ok(_) => break,
                    Err(err) => {
                        let time = 2 ^ retry_count;
                        retry_count += 1;
                        println!(
                            "Error connecting to discord: {}, retrying in {} seconds",
                            err.to_string(),
                            time
                        );

                        tokio::time::sleep(std::time::Duration::from_secs(time)).await;
                    }
                }
            }

            Ok(())
        }

        let join_task = tokio::spawn(wrap_client(client));

        // Block on the client task until it is ready or it has errored and yielded
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let join_task = Abortable::new(join_task, abort_registration);
        *service.ready_abort.lock().unwrap() = Some(abort_handle);

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

    async fn message(
        self: &Arc<Self>,
        channel_id: Self::ChannelId,
        id: Self::MessageId,
    ) -> Result<Arc<Self::Message>> {
        let message = match self.cache_and_http().cache.message(channel_id, id).await {
            Some(message) => message,
            None => {
                self.cache_and_http()
                    .http
                    .get_message(channel_id, id)
                    .await?
            }
        };

        Ok(Arc::new(message::DiscordMessage::new(
            message,
            self.clone(),
        )))
    }

    async fn channel(self: &Arc<Self>, id: Self::ChannelId) -> Result<Arc<Self::Channel>> {
        let channel = match self.cache_and_http().cache.channel(id).await {
            Some(channel) => channel,
            None => self.cache_and_http().http.get_channel(id).await?,
        };

        Ok(Arc::new(channel::DiscordChannel::new(
            channel,
            self.clone(),
        )))
    }

    async fn user(self: &Arc<Self>, id: u64) -> Result<Arc<Self::User>> {
        let mut lru_cache = self.user_cache.lock().await;

        if let Some(user) = lru_cache.get(&id) {
            return Ok(user.clone());
        }

        let user = match self.cache_and_http().cache.user(id).await {
            Some(user) => user,
            None => self.cache_and_http().http.get_user(id).await?,
        };

        let user = Arc::new(user::DiscordUser::new(user, self.clone()));

        lru_cache.put(id, user.clone());

        Ok(user)
    }

    async fn find_user(self: &Arc<Self>, channel_id: u64, find: &str) -> Result<Arc<Self::User>> {
        let find = find.trim();

        if let Some(id) = serenity::utils::parse_username(find).or(u64::from_str(find).ok()) {
            let user = match self.cache_and_http().cache.user(id).await {
                Some(channel) => channel,
                None => self.cache_and_http().http.get_user(id).await?,
            };

            Ok(Arc::new(user::DiscordUser::new(user, self.clone())))
        } else {
            let channel = self.find_channel(channel_id).await?;

            // Search for the member manually
            if let Some((member, _)) = channel
                .server()
                .await?
                .guild()
                .members_username_containing(find, false, true)
                .await
                .first()
            {
                return Ok(Arc::new(user::DiscordUser::new(
                    member.user.clone(),
                    self.clone(),
                )));
            }

            // TODO: Look in caches for name matches?
            return Err(anyhow!("unable to parse \"{}\" as a discord user", find));
        }
    }

    async fn react(
        self: &Arc<Self>,
        channel_id: u64,
        message_id: u64,
        reaction: String,
    ) -> Result<()> {
        self.cache_and_http()
            .http
            .create_reaction(channel_id, message_id, &ReactionType::Unicode(reaction))
            .await?;

        Ok(())
    }
}

impl DiscordService {
    fn cache_and_http(&self) -> Arc<CacheAndHttp> {
        self.cache_and_http.load_full().unwrap()
    }

    async fn find_channel(self: &Arc<Self>, id: u64) -> Result<Arc<channel::DiscordChannel>> {
        let channel = match self.cache_and_http().cache.channel(id).await {
            Some(channel) => channel,
            None => self.cache_and_http().http.get_channel(id).await?,
        };

        Ok(Arc::new(channel::DiscordChannel::new(
            channel,
            self.clone(),
        )))
    }
}

#[derive(Error, Debug)]
pub enum DiscordError {
    #[error("the channel does not have a guild")]
    NoChannelGuild,
    #[error("cache miss")]
    CacheMiss,
}
