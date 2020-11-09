use anyhow::Result;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::{
    modules::Modules,
    services::{Message, Service, Services},
};

pub struct Bot {
    ctx: RwLock<Option<Arc<BotContext>>>,
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
    pub async fn init() -> Result<Arc<Bot>> {
        Ok(Arc::new(Bot {
            ctx: RwLock::new(None),
        }))
    }

    pub fn set_ctx(&self, ctx: Arc<BotContext>) {
        *self.ctx.write() = Some(ctx);
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
