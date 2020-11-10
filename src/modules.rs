use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod shell;

use crate::{
    bot::Bot,
    config::Config,
    services::{Message, Service},
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
                        self.$module_ident.module().message(msg.clone()).await;
                    }
                )+
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
    const NAME: &'static str;

    type ModuleConfig: Clone + Deserialize<'static> + Serialize + std::fmt::Debug;
    type ModuleSettings;

    async fn load(bot: Arc<Bot>, config: Self::ModuleConfig) -> Result<Arc<Self>>;

    // TODO: Move message to type alias when impl's inside type aliases becomes stable
    async fn message(&self, msg: Arc<dyn Message<impl Service>>);
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
    Shell,
}

modules_loader! {
    Modules,

    shell => (shell::ShellModule, ())
}
