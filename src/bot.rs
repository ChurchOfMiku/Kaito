use anyhow::Result;
use parking_lot::RwLock;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    modules::Modules,
    services::{Message, Service, Services},
};

pub struct Bot {
    ctx: RwLock<Option<Arc<BotContext>>>,
    root_path: PathBuf,
}

macro_rules! get_ctx {
    ($self:expr) => {
        match &*$self.ctx.read() {
            Some(c) => c.clone(),
            None => return,
        }
    };
}

impl Bot {
    pub async fn init(root_path: PathBuf) -> Result<Arc<Bot>> {
        Ok(Arc::new(Bot {
            ctx: RwLock::new(None),
            root_path,
        }))
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn set_ctx(&self, ctx: Arc<BotContext>) {
        *self.ctx.write() = Some(ctx);
    }

    pub fn get_ctx(&self) -> Arc<BotContext> {
        self.ctx.read().clone().expect("bot context")
    }

    pub async fn message(&self, msg: Arc<dyn Message<impl Service>>) {
        let ctx = get_ctx!(self);

        ctx.modules().message(msg).await;
    }
}

pub struct BotContext {
    bot: Arc<Bot>,
    modules: Arc<Modules>,
    services: Arc<Services>,
}

#[allow(dead_code)]
impl BotContext {
    pub fn new(bot: Arc<Bot>, modules: Arc<Modules>, services: Arc<Services>) -> Arc<BotContext> {
        Arc::new(BotContext {
            bot,
            modules,
            services,
        })
    }

    pub fn bot(&self) -> &Arc<Bot> {
        &self.bot
    }

    pub fn modules(&self) -> &Arc<Modules> {
        &self.modules
    }

    pub fn services(&self) -> &Arc<Services> {
        &self.services
    }
}
