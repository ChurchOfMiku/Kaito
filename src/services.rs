use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod discord;

use crate::bot::Bot;

pub enum ServiceKind {
    Discord,
}

#[async_trait]
pub trait Service: 'static + Sized + Send + Sync {
    const KIND: ServiceKind;
    const NAME: &'static str;

    type ServiceConfig: Clone + Deserialize<'static> + Serialize + std::fmt::Debug;
    type Message: Message<Self>;
    type User: User<Self>;
    type Channel: Channel<Self>;
    type Server: Server<Self>;

    async fn init(bot: Arc<Bot>, config: Self::ServiceConfig) -> Result<Arc<Self>>;
    async fn unload(&self) -> Result<()>;
}

#[async_trait]
pub trait Message<S: Service>: Send + Sync {
    fn author(&self) -> &Arc<S::User>;
    async fn channel(&self) -> Result<Arc<S::Channel>>;
    fn content(&self) -> &str;
    fn service(&self) -> &Arc<S>;
}

pub trait User<S: Service>: Send + Sync {
    fn id(&self) -> UserId;
    fn name(&self) -> &str;
    fn avatar(&self) -> &Option<String> {
        &None
    }
    fn bot(&self) -> Option<bool> {
        None
    }
    fn service(&self) -> &Arc<S>;
}

#[async_trait]
pub trait Channel<S: Service>: Send + Sync {
    fn id(&self) -> ChannelId;
    fn name(&self) -> String;
    async fn server(&self) -> Result<Arc<S::Server>>;
    fn service(&self) -> &Arc<S>;
}

pub trait Server<S: Service>: Send + Sync {
    fn id(&self) -> ServerId;
    fn name(&self) -> &str;
    fn service(&self) -> &Arc<S>;
}

pub enum UserId {
    Discord(u64),
}

pub enum ChannelId {
    Discord(u64),
}

pub enum ServerId {
    Discord(u64),
}
