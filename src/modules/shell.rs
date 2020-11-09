use anyhow::Result;
use std::sync::Arc;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    services::{Message, Service},
};

pub struct ShellModule {}

#[async_trait]
impl Module for ShellModule {
    const KIND: ModuleKind = ModuleKind::Shell;
    const NAME: &'static str = "Shell";

    type ModuleConfig = ();

    async fn load(_bot: Arc<Bot>, _config: ()) -> Result<Arc<Self>> {
        Ok(Arc::new(ShellModule {}))
    }

    async fn message(&self, _msg: Arc<dyn Message<impl Service>>) {}
}
