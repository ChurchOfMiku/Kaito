use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};

pub mod discord;

use crate::{
    bot::Bot,
    config::ConfigServices,
    message::{Attachment, MessageSettings, ToMessageContent},
};

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

            pub async fn send_message<'a, C>(&self, channel_id: ChannelId, content: C, settings: MessageSettings) -> Result<Arc<dyn Message<impl Service>>>
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

                            let msg: Arc<dyn Message<_>> = channel.send(content, settings).await?;
                            Ok(msg)
                        }
                    ),+
                }
            }

            pub async fn send_typing(&self, channel_id: ChannelId) -> Result<()> {
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

                            channel.send_typing().await
                        }
                    ),+
                }
            }

            #[allow(unreachable_patterns)]
            pub async fn edit_message<'a, C>(&self, channel_id: ChannelId, message_id: MessageId, content: C) -> Result<()>
            where
                C: ToMessageContent<'a>
            {
                match channel_id {
                    $(
                        ChannelId::$service_module_ident (id) => {
                            let message_id = match message_id {
                                MessageId::$service_module_ident(msg_id) => msg_id,
                                _ => unreachable!()
                            };

                            self.$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service()
                                .message(id, message_id).await?.edit(content).await
                        }
                    ),+
                }
            }

            #[allow(unreachable_patterns)]
            pub async fn delete_message(&self, channel_id: ChannelId, message_id: MessageId) -> Result<()> {
                match channel_id {
                    $(
                        ChannelId::$service_module_ident (id) => {
                            let message_id = match message_id {
                                MessageId::$service_module_ident(msg_id) => msg_id,
                                _ => unreachable!()
                            };

                            self.$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service()
                                .message(id, message_id).await?.delete().await
                        }
                    ),+
                }
            }


            pub async fn user(&self, user_id: UserId) -> Result<Arc<dyn User<impl Service>>> {
                match user_id {
                    $(
                        UserId::$service_module_ident(id) => {
                            let user: Arc<<$service as Service>::User> = self.$service_ident.as_ref()
                            .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                            .service()
                            .user(id)
                            .await?;

                            Ok(user)
                        }
                    ),+
                }
            }

            pub async fn channel(&self, channel_id: ChannelId) -> Result<Arc<dyn Channel<impl Service>>> {
                match channel_id {
                    $(
                        ChannelId::$service_module_ident(id) => {
                            let channel: Arc<<$service as Service>::Channel> = self.$service_ident.as_ref()
                            .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                            .service()
                            .channel(id)
                            .await?;

                            Ok(channel)
                        }
                    ),+
                }
            }

            #[allow(unreachable_patterns)]
            pub async fn message(&self, channel_id: ChannelId, message_id: MessageId) -> Result<Arc<dyn Message<impl Service>>> {
                match (channel_id, message_id) {
                    $(
                        (ChannelId::$service_module_ident(chan_id), MessageId::$service_module_ident(msg_id)) => {
                            let channel: Arc<<$service as Service>::Message> = self.$service_ident.as_ref()
                            .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                            .service()
                            .message(chan_id, msg_id)
                            .await?;

                            Ok(channel)
                        },
                    )+
                    _ => Err(anyhow::anyhow!("channel id and message id does not belong to the same service"))
                }
            }

            #[allow(unreachable_patterns)]
            pub async fn find_user(&self, channel_id: ChannelId, find: &str) -> Result<Arc<dyn User<impl Service>>> {
                if let Some(sep) = find.find(':') {
                    let (before, after) = find.split_at(sep);
                    let after = &after[1..];

                    match before {
                        $(
                            <$service as Service>::ID | <$service as Service>::ID_SHORT => {
                                let user = self.$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service()
                                    .find_user(match channel_id {
                                          ChannelId::$service_module_ident(id) => id,
                                        _ => panic!()
                                    }, after).await?;

                                return Ok(user)
                            },
                        ),+
                        _ => {}
                    }
                }

                Ok(
                    match channel_id {
                        $(
                            ChannelId::$service_module_ident(id) => self
                            .$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service()
                            .find_user(id, find)
                            .await?,
                        ),+
                    }
                )
            }

            #[allow(unreachable_patterns)]
            pub async fn react(&self, channel_id: ChannelId, message_id: MessageId, reaction: String) -> Result<()> {
                match channel_id {
                    $(
                        ChannelId::$service_module_ident(channel_id) => {
                            let message_id = match message_id {
                                MessageId::$service_module_ident(msg_id) => msg_id,
                                _ => unreachable!()
                            };

                            self.$service_ident
                                .as_ref()
                                .ok_or(anyhow!("service {} has not been started", stringify!($service_module_ident)))?
                                .service()
                                .react(channel_id, message_id, reaction).await
                        }
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
        pub enum MessageId {
            $($service_module_ident (<$service as Service>::MessageId)),+
        }

        service_id_functions!{MessageId, MessageId, $(($service_module_ident, $service)),+}


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
        pub enum UserId {
            $($service_module_ident (<$service as Service>::UserId)),+
        }

        service_id_functions!{UserId, UserId, $(($service_module_ident, $service)),+}

        #[derive(Copy, Clone, Hash, PartialEq)]
        pub enum ServiceKind {
            $($service_module_ident),+
        }

        impl ServiceKind {
            #[allow(dead_code)]
            pub fn from_str(s: &str) -> Option<ServiceKind> {
                match s {
                    $(stringify!($service_ident) => Some(ServiceKind::$service_module_ident),)+
                    _ => None
                }
            }

            pub fn supports_feature(&self, feature: ServiceFeatures) -> bool {
                match self {
                    $(
                        ServiceKind::$service_module_ident => <$service as Service>::supports_feature(feature)
                    ),+
                }
            }
        }

        $(
            impl std::convert::TryInto<<$service as Service>::UserId> for UserId {
                type Error = &'static str;

                fn try_into(self: UserId) -> Result<<$service as Service>::UserId, Self::Error> {
                    #[allow(unreachable_patterns)]
                    match self {
                        UserId::$service_module_ident(id) => Ok(id),
                        _ => Err("user id belongs to another service")
                    }
                }
            }
        )+
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

    type MessageId;
    type ChannelId;
    type ServerId;
    type UserId;

    async fn init(bot: Arc<Bot>, config: Self::ServiceConfig) -> Result<Arc<Self>>;
    async fn unload(&self) -> Result<()>;

    async fn current_user(self: &Arc<Self>) -> Result<Arc<Self::User>>;
    async fn message(
        self: &Arc<Self>,
        channel_id: Self::ChannelId,
        id: Self::MessageId,
    ) -> Result<Arc<Self::Message>>;
    async fn channel(self: &Arc<Self>, id: Self::ChannelId) -> Result<Arc<Self::Channel>>;
    async fn user(self: &Arc<Self>, get_user: Self::UserId) -> Result<Arc<Self::User>>;
    async fn find_user(
        self: &Arc<Self>,
        channel_id: Self::ChannelId,
        find: &str,
    ) -> Result<Arc<Self::User>>;

    async fn react(
        self: &Arc<Self>,
        channel_id: Self::MessageId,
        msg_id: Self::MessageId,
        reaction: String,
    ) -> Result<()>;

    fn kind(&self) -> ServiceKind {
        Self::KIND
    }

    fn supports_feature(feature: ServiceFeatures) -> bool {
        Self::FEATURES.contains(feature)
    }
}

bitflags! {
    pub struct ServiceFeatures: u32 {
        const EDIT = 1;
        const EMBED = 1 << 1;
        const REACT = 1 << 2;
        const VOICE = 1 << 3;
        const MARKDOWN = 1 << 4;
    }
}

#[async_trait]
pub trait Message<S: Service>: Send + Sync {
    fn author(&self) -> &Arc<S::User>;
    async fn channel(&self) -> Result<Arc<S::Channel>>;
    async fn edit<'a, C>(&self, content: C) -> Result<()>
    where
        Self: Sized,
        C: ToMessageContent<'a>;
    async fn delete(&self) -> Result<()>;
    fn content(&self) -> &str;
    fn attachments(&self) -> &[Arc<Attachment>];
    fn service(&self) -> &Arc<S>;
    fn id(&self) -> MessageId;
}

pub trait User<S: Service>: Send + Sync {
    fn id(&self) -> UserId;
    fn name(&self) -> &str;
    fn nick(&self) -> &str;
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
    async fn send<'a, C>(&self, content: C, settings: MessageSettings) -> Result<Arc<S::Message>>
    where
        Self: Sized,
        C: ToMessageContent<'a>;
    async fn server(&self) -> Result<Arc<S::Server>>;
    async fn send_typing(&self) -> Result<()>;
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
