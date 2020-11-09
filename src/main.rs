#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate serde_derive;

use anyhow::Result;
use std::env;

mod bot;
mod config;
mod modules;
mod services;

use services::Service;

async fn run() -> Result<()> {
    let config = config::load_config(&env::current_dir()?.join("config.toml"))?;

    let bot = bot::Bot::init().await?;
    let modules = modules::Modules::init(bot.clone(), &config).await?;

    if let Some(discord_config) = config.services.discord {
        let _service = services::discord::DiscordService::init(bot, discord_config).await?;
    }

    println!("Everything is online");

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        println!("Error: {}", err.to_string());
    }

    loop {}
}
