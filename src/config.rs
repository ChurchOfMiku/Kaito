use anyhow::Result;
use std::{collections::HashMap, fs, path::Path};

use crate::services::discord::DiscordServiceConfig;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Config {
    pub services: ConfigServices,
    pub user_roles: Option<HashMap<String, String>>,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct ConfigServices {
    pub discord: Option<DiscordServiceConfig>,
}

pub fn load_config(path: &Path) -> Result<Config> {
    let contents = fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}
