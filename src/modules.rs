use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod lua;

use crate::{
    bot::Bot,
    config::Config,
    services::{ChannelId, Message, ServerId, Service, User},
    settings::Settings,
};

macro_rules! modules_loader {
    ($modules_struct:ident, $($module_ident:ident => ($module:ty, $module_config:tt)),*) => {
        pub struct $modules_struct {
            $(pub $module_ident: ModuleWrapper<$module>),+
        }

        impl $modules_struct {
            #[allow(unused_variables)]
            pub async fn init(bot: Arc<Bot>, config: &Config) -> Result<Arc<$modules_struct>> {
                Ok(Arc::new($modules_struct {
                    $(
                        $module_ident: ModuleWrapper::new(modules_loader! {__init, $module, bot.clone(), config, $module_config})
                    ),+
                }))
            }

            #[allow(dead_code)]
            pub async fn message(&self, msg: Arc<dyn Message<impl Service>>) {
                $(
                    if self.$module_ident.is_enabled() {
                        if let Err(err) = self.$module_ident.module().message(msg.clone()).await {
                            println!("error during executing module {}: {}", self.$module_ident.module().name(), err.to_string())
                        };
                    }
                )+
            }

            pub async fn message_update(&self, msg: Arc<dyn Message<impl Service>>, old_msg: Option<Arc<dyn Message<impl Service>>>) {
                $(
                    if self.$module_ident.is_enabled() {
                        if let Err(err) = self.$module_ident.module().message_update(msg.clone(), old_msg.clone()).await {
                            println!("error during executing module {}: {}", self.$module_ident.module().name(), err.to_string())
                        };
                    }
                )+
            }

            #[allow(dead_code)]
            pub async fn reaction(&self, msg: Arc<dyn Message<impl Service>>, reactor: Arc<dyn User<impl Service>>, reaction: String, remove: bool) {
                $(
                    if self.$module_ident.is_enabled() {
                        if let Err(err) = self.$module_ident.module().reaction(msg.clone(), reactor.clone(), reaction.clone(), remove).await {
                            println!("error during executing module {}: {}", self.$module_ident.module().name(), err.to_string())
                        };
                    }
                )+
            }

            pub fn get_settings(&self, name: &str) -> Option<Arc<dyn Settings>> {
                match name {
                    $(
                        <$module>::ID => Some(self.$module_ident.module().settings().clone() as Arc<_>),
                    ),+
                    _ => None
                }
            }

            // Join all the unload functions of the modules and return the first error if any
            pub async fn unload(&self) -> Result<()> {
                let (
                    $($module_ident,)+
                ) = futures::join!(
                    $(
                        self.$module_ident.module().unload()
                    ),+
                );


                match None.$(or_else(|| $module_ident.err())).+ {
                    Some(err) => Err(err),
                    None => Ok(())
                }
            }
        }
    };
    (__init, $module:ty, $bot:expr, $config:expr, ()) => {
        <$module>::load($bot, ()).await?
    };
    (__init, $module:ty, $bot:expr, $config:expr, $config_ident:ident) => {
        <$module>::load($bot, $config.$config_ident).await?
    };
}

#[async_trait]
pub trait Module: 'static + Send + Sync + Sized {
    const KIND: ModuleKind;
    const ID: &'static str;
    const NAME: &'static str;

    type ModuleConfig: Clone + Deserialize<'static> + Serialize + std::fmt::Debug;
    type ModuleSettings: Settings;

    async fn load(bot: Arc<Bot>, config: Self::ModuleConfig) -> Result<Arc<Self>>;
    async fn unload(&self) -> Result<()>;

    // TODO: Move message to type alias when impl's inside type aliases becomes stable
    async fn message(&self, msg: Arc<dyn Message<impl Service>>) -> Result<()>;
    async fn message_update(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        old_msg: Option<Arc<dyn Message<impl Service>>>,
    ) -> Result<()>;
    async fn reaction(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        reactor: Arc<dyn User<impl Service>>,
        reaction: String,
        remove: bool,
    ) -> Result<()>;

    async fn enabled(&self, server_id: ServerId, channel_id: ChannelId) -> Result<bool>;

    fn kind(&self) -> ModuleKind {
        Self::KIND
    }
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn settings(&self) -> &Arc<Self::ModuleSettings>;
}

pub struct ModuleWrapper<M: Module> {
    module: Arc<M>,
}

impl<M: Module> ModuleWrapper<M> {
    pub fn new(module: Arc<M>) -> ModuleWrapper<M> {
        ModuleWrapper { module }
    }

    pub fn is_enabled(&self) -> bool {
        true
    }

    pub fn module(&self) -> &Arc<M> {
        &self.module
    }
}

pub enum ModuleKind {
    Lua,
}

modules_loader! {
    Modules,

    lua => (lua::LuaModule, ())
}
