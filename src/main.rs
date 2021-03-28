#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate serde_derive;

use anyhow::Result;
use std::{env, path::PathBuf};

#[macro_use]
mod settings;

mod bot;
mod config;
mod message;
mod modules;
mod services;
mod utils;

async fn run() -> Result<()> {
    let config_path = env::var("KAITO_CONFIG_FILE")
        .map(|p| PathBuf::from(p))
        .or_else(|_| env::current_dir().map(|p| p.join("config.toml")))?;

    let data_path = env::var("KAITO_DATA_PATH")
        .map(|p| PathBuf::from(p))
        .or_else(|_| env::current_dir().map(|p| p.join("data")))?;

    let share_path = env::var("KAITO_SHARE_PATH")
        .map(|p| PathBuf::from(p))
        .or_else(|_| env::current_dir())?;

    let config = config::load_config(&config_path)?;

    if !data_path.is_dir() {
        std::fs::create_dir_all(&data_path)?;
    }

    let bot = bot::Bot::init(data_path, share_path, &config).await?;
    let modules = modules::Modules::init(bot.clone(), &config).await?;
    let services = services::Services::init(bot.clone(), &config.services).await?;
    let ctx = bot::BotContext::new(bot.clone(), modules, services);
    bot.set_ctx(ctx);

    println!("Everything is online");

    tokio::signal::ctrl_c().await?;
    println!("Exit signal received, shutting down...");
    bot.get_ctx().shutdown().await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    graphicsmagick::initialize();

    if let Err(err) = run().await {
        println!("Error: {}", err.to_string());
    }
}
