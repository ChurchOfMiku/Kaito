use anyhow::Result;
use std::sync::Arc;

pub mod discord;

pub enum ServiceKind {
    Discord,
}

#[async_trait]
pub trait Service: 'static + Sized + Send + Sync {
    const KIND: ServiceKind;
    const NAME: &'static str;

    type ServiceConfig;
    type Message: Message<Self>;
    type User: User<Self>;
    type Channel: Channel<Self>;
    type Server: Server<Self>;

    async fn init(config: Self::ServiceConfig) -> Result<Arc<Self>>;
    async fn unload(&self) -> Result<()>;
}

pub trait Message<S: Service> {
    fn author(&self) -> &Arc<S::User>;
    fn channel(&self) -> &Arc<S::Channel>;
    fn service(&self) -> &Arc<S>;
}

pub trait User<S: Service> {
    fn service(&self) -> &Arc<S>;
}

pub trait Channel<S: Service> {
    fn server(&self) -> &Arc<S::Server>;
    fn service(&self) -> &Arc<S>;
}

pub trait Server<S: Service> {
    fn service(&self) -> &Arc<S>;
}
