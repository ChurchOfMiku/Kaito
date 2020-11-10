use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod discord;

use crate::{bot::Bot, config::ConfigServices};

macro_rules! services {
    ($services_struct:ident, $($service_ident:ident => $service:ty),*) => {
        pub struct $services_struct {
            $(pub $service_ident: Option<ServiceWrapper<$service>>),+
        }

        impl $services_struct {
            #[allow(unused_variables)]
            pub async fn init(bot: Arc<Bot>, config: &ConfigServices) -> Result<Arc<$services_struct>> {
                Ok(Arc::new($services_struct {
                    $(
                        $service_ident: if let Some(service_config) = config.$service_ident.clone() {
                            Some(ServiceWrapper::new(<$service>::init(bot, service_config).await?))
                        } else {
                            None
                        }
                    ),+
                }))
            }
        }
    };
}

pub enum ServiceKind {
    Discord,
}

#[async_trait]
pub trait Service: 'static + Sized + Send + Sync {
    const KIND: ServiceKind;
    const NAME: &'static str;
    const FEATURES: ServiceFeatures;

    type ServiceConfig: Clone + Deserialize<'static> + Serialize + std::fmt::Debug;
    type Message: Message<Self>;
    type User: User<Self>;
    type Channel: Channel<Self>;
    type Server: Server<Self>;

    async fn init(bot: Arc<Bot>, config: Self::ServiceConfig) -> Result<Arc<Self>>;
    async fn unload(&self) -> Result<()>;
}

bitflags! {
    pub struct ServiceFeatures: u32 {
        const EMBEDS = 1;
        const REACTIONS = 1 << 1;
        const VOICE = 1 << 2;
    }
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

pub struct ServiceWrapper<S: Service> {
    service: Arc<S>,
}

impl<S: Service> ServiceWrapper<S> {
    pub fn new(service: Arc<S>) -> ServiceWrapper<S> {
        ServiceWrapper { service }
    }

    #[allow(dead_code)]
    pub fn service(&self) -> &Arc<S> {
        &self.service
    }
}

services! {
    Services,
    discord => discord::DiscordService
}
