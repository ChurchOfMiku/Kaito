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
        enable: bool => (true, SettingFlags::SERVER_OVERRIDE, "Enable the shell module", []),
        prefix: String => ("$".into(), SettingFlags::empty(), "Set the message prefix for shell commands", [max_len => 8])
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

    fn enabled(&self) -> bool {
        *self.settings.enable.value()
    }
}
