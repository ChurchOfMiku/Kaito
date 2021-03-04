use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};

pub mod discord;

use crate::{bot::Bot, config::ConfigServices, message::ToMessageContent};

macro_rules! service_id_functions {
    ($id:ident, $service_id:ident, $(($service_module_ident:ident, $service:ty)),+) => {
        #[allow(dead_code)]
        impl $id {
            pub fn to_str(&self) -> String {
                match self {
                    $($id::$service_module_ident(id) => format!("{}:{}", <$service as Service>::ID, id)),+
                }
            }

            pub fn to_short_str(&self) -> String {
                match self {
                    $($id::$service_module_ident (id) => format!("{}:{}", <$service as Service>::ID_SHORT, id)),+
                }
            }

            pub fn from_str(text: &str) -> Result<$id> {
                if let Some(sep) = text.find(':') {
                    let (before, after) = text.split_at(sep);
                    let after = &after[1..];

                    match before {
                        $(
                            <$service as Service>::ID | <$service as Service>::ID_SHORT => {
                                let id = <$service as Service>::$service_id::from_str(after)?;
                                return Ok($id::$service_module_ident(id));
                            },
                        ),+
                        _ => return Err(anyhow!("unknown service \"{}\"", before))
                    }
                }

                Err(anyhow!("id seperator missing: \"{}\"", text))
            }

            pub fn service_kind(&self) -> ServiceKind {
                match self {
                    $($id::$service_module_ident (_) => ServiceKind::$service_module_ident),+
                }
            }
        }
    };
}

macro_rules! services {
    ($services_struct:ident, $($service_ident:ident => ($service_module_ident:ident, $service:ty)),*) => {
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

            pub async fn send_message<'a, C>(&self, channel_id: ChannelId, content: C) -> Result<()>
            where
                C: ToMessageContent<'a>
            {
                match channel_id {
                    $(
                        ChannelId::$service_module_ident (id) => {
                            let channel = self
                                .$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service()
                                .channel(id)
                                .await?;

                            channel.send(content).await?;
                        }
                    ),+
                }

                Ok(())
            }

            pub async fn find_user(&self, service: ServiceKind, find: &str) -> Result<Arc<dyn User<impl Service>>> {
                if let Some(sep) = find.find(':') {
                    let (before, after) = find.split_at(sep);
                    let after = &after[1..];

                    match before {
                        $(
                            <$service as Service>::ID | <$service as Service>::ID_SHORT => {
                                let user = self
                                    .get_service_from_kind(<$service as Service>::KIND)?
                                    .find_user(after).await?;

                                return Ok(user)
                            },
                        ),+
                        _ => {}
                    }
                }

                Ok(
                    self
                        .get_service_from_kind(service)?
                        .find_user(find)
                        .await?
                )
            }

            pub fn get_service_from_kind(&self, kind: ServiceKind) -> Result<&Arc<impl Service>> {
                match kind {
                    $(
                        ServiceKind::$service_module_ident => {
                            return Ok(self.$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service())
                        },
                    ),+
                }
            }

            pub fn id_from_kind(kind: ServiceKind) -> &'static str {
                match kind {
                    $(ServiceKind::$service_module_ident => <$service as Service>::ID),+
                }
            }
        }

        #[derive(Copy, Clone, Hash, Eq, PartialEq)]
        pub enum ChannelId {
            $($service_module_ident (<$service as Service>::ChannelId)),+
        }

        service_id_functions!{ChannelId, ChannelId, $(($service_module_ident, $service)),+}

        #[derive(Copy, Clone, Hash, Eq, PartialEq)]
        pub enum ServerId {
            $($service_module_ident (<$service as Service>::ServerId)),+
        }

        service_id_functions!{ServerId, ServerId, $(($service_module_ident, $service)),+}

        #[derive(Copy, Clone, Hash, Eq, PartialEq)]
        pub enum ServiceUserId {
            $($service_module_ident (<$service as Service>::UserId)),+
        }

        service_id_functions!{ServiceUserId, UserId, $(($service_module_ident, $service)),+}

        #[derive(Copy, Clone, Hash, PartialEq)]
        pub enum ServiceKind {
            $($service_module_ident),+
        }

        impl ServiceKind {
            pub fn from_str(s: &str) -> Option<ServiceKind> {
                match s {
                    $(stringify!($service_ident) => Some(ServiceKind::$service_module_ident),)+
                    _ => None
                }
            }
        }
    };
}

#[async_trait]
pub trait Service: 'static + Sized + Send + Sync {
    const KIND: ServiceKind;
    const ID: &'static str;
    const ID_SHORT: &'static str;
    const NAME: &'static str;
    const FEATURES: ServiceFeatures;

    type ServiceConfig: Clone + Deserialize<'static> + Serialize + std::fmt::Debug;
    type Message: Message<Self>;
    type User: User<Self>;
    type Channel: Channel<Self>;
    type Server: Server<Self>;

    type ChannelId;
    type ServerId;
    type UserId;

    async fn init(bot: Arc<Bot>, config: Self::ServiceConfig) -> Result<Arc<Self>>;
    async fn unload(&self) -> Result<()>;

    async fn current_user(self: &Arc<Self>) -> Result<Arc<Self::User>>;
    async fn channel(self: &Arc<Self>, id: Self::ChannelId) -> Result<Arc<Self::Channel>>;
    async fn find_user(self: &Arc<Self>, find: &str) -> Result<Arc<Self::User>>;

    fn kind(&self) -> ServiceKind {
        Self::KIND
    }
}

bitflags! {
    pub struct ServiceFeatures: u32 {
        const EMBEDS = 1;
        const REACTIONS = 1 << 1;
        const VOICE = 1 << 2;
        const MARKDOWN = 1 << 3;
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
    fn id(&self) -> ServiceUserId;
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
    async fn send<'a, C>(&self, content: C) -> Result<()>
    where
        Self: Sized,
        C: ToMessageContent<'a>;
    async fn server(&self) -> Result<Arc<S::Server>>;
    fn service(&self) -> &Arc<S>;
}

pub trait Server<S: Service>: Send + Sync {
    fn id(&self) -> ServerId;
    fn name(&self) -> &str;
    fn service(&self) -> &Arc<S>;
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
    discord => (Discord, discord::DiscordService)
}
