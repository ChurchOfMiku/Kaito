#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate serde_derive;

use anyhow::Result;
use std::env;

#[macro_use]
mod settings;

mod bot;
mod config;
mod message;
mod modules;
mod services;
mod utils;

async fn run() -> Result<()> {
    let config = config::load_config(&env::current_dir()?.join("config.toml"))?;

    let bot = bot::Bot::init(env::current_dir()?, &config).await?;
    let modules = modules::Modules::init(bot.clone(), &config).await?;
    let services = services::Services::init(bot.clone(), &config.services).await?;
    let ctx = bot::BotContext::new(bot.clone(), modules, services);
    bot.set_ctx(ctx);

    println!("Everything is online");

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        println!("Error: {}", err.to_string());
    }

    loop {
        tokio::task::yield_now().await;
        std::thread::yield_now();
    }
}
