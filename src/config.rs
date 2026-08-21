use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use teloxide::types::{ChatId, UserId};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigLoadError {
    #[error("failed read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parsing error: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Deserialize)]
pub struct Config {
    pub token: String,
    pub support_chat: ChatId,
    pub support_categories: Vec<Arc<String>>,
    pub support_categories_layout: Vec<Vec<usize>>,
    #[serde(default)]
    pub super_admins: BTreeSet<UserId>,
    #[serde(default = "default_storage_path")]
    pub storage: PathBuf,
}

fn default_storage_path() -> PathBuf {
    "/var/oktebot".into()
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, ConfigLoadError> {
        Ok(toml::from_str(fs::read_to_string(path)?.as_str())?)
    }
}
