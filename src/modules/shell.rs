use anyhow::Result;
use std::sync::Arc;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    services::{Message, Service},
    settings::prelude::*,
};

pub struct ShellModule {
    settings: Arc<ShellModuleSettings>,
}

settings! {
    ShellModuleSettings,
    {
        enable: bool => (true, SettingFlags::empty(), "Enable the shell module", [])
    }
}

#[async_trait]
impl Module for ShellModule {
    const KIND: ModuleKind = ModuleKind::Shell;
    const NAME: &'static str = "Shell";

    type ModuleConfig = ();
    type ModuleSettings = ShellModuleSettings;

    async fn load(_bot: Arc<Bot>, _config: ()) -> Result<Arc<Self>> {
        Ok(Arc::new(ShellModule {
            settings: ShellModuleSettings::create()?,
        }))
    }

    async fn message(&self, _msg: Arc<dyn Message<impl Service>>) {}
}
