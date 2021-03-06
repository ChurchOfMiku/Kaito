use anyhow::Result;
use arc_swap::ArcSwapOption;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub mod db;

use crate::{
    config::Config,
    modules::Modules,
    services::{ChannelId, Message, MessageId, ServerId, Service, Services, User},
};
use db::BotDb;

pub const ROLES: &[&'static str] = &["guest", "trusted", "admin", "root"];
pub const DEFAULT_ROLE: &'static str = ROLES[0];

pub struct Bot {
    ctx: ArcSwapOption<BotContext>,
    db: Arc<BotDb>,
    data_path: PathBuf,
    share_path: PathBuf,
}

macro_rules! get_ctx {
    ($self:expr) => {
        match &*$self.ctx.load() {
            Some(c) => c.clone(),
            None => return,
        }
    };
}

impl Bot {
    pub async fn init(
        data_path: PathBuf,
        share_path: PathBuf,
        config: &Config,
    ) -> Result<Arc<Bot>> {
        Ok(Arc::new(Bot {
            ctx: ArcSwapOption::default(),
            db: BotDb::new(&data_path, &share_path, config).await?,
            data_path,
            share_path,
        }))
    }

    pub fn data_path(&self) -> &Path {
        &self.data_path
    }

    pub fn share_path(&self) -> &Path {
        &self.share_path
    }

    pub fn db(&self) -> &Arc<BotDb> {
        &self.db
    }

    pub fn set_ctx(&self, ctx: Arc<BotContext>) {
        self.ctx.store(Some(ctx));
    }

    pub fn get_ctx(&self) -> Arc<BotContext> {
        loop {
            if let Some(ctx) = self.ctx.load_full().map(|ctx| ctx.clone()) {
                return ctx;
            }

            std::thread::yield_now();
        }
    }

    pub async fn message(&self, msg: Arc<dyn Message<impl Service>>) {
        let ctx = get_ctx!(self);

        ctx.modules().message(msg).await;
    }

    pub async fn message_update(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        old_msg: Option<Arc<dyn Message<impl Service>>>,
    ) {
        let ctx = get_ctx!(self);

        ctx.modules().message_update(msg, old_msg).await;
    }

    pub async fn message_delete(
        &self,
        server_id: Option<ServerId>,
        channel_id: ChannelId,
        message_id: MessageId,
    ) {
        let ctx = get_ctx!(self);

        ctx.modules()
            .message_delete(server_id, channel_id, message_id)
            .await;
    }

    pub async fn reaction(
        &self,
        msg: Arc<dyn Message<impl Service>>,
        reactor: Arc<dyn User<impl Service>>,
        reaction: String,
        remove: bool,
    ) {
        let ctx = get_ctx!(self);

        ctx.modules().reaction(msg, reactor, reaction, remove).await;
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

    pub async fn shutdown(&self) -> Result<()> {
        self.modules().unload().await?;

        Ok(())
    }
}
