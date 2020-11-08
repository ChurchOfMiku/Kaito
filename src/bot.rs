use anyhow::Result;
use std::sync::Arc;

use crate::services::{Message, Service};

pub struct Bot {}

impl Bot {
    pub async fn init() -> Result<Arc<Bot>> {
        Ok(Arc::new(Bot {}))
    }

    pub async fn message(&self, msg: Arc<dyn Message<impl Service>>) {
        println!("{}", msg.content());
    }
}
