use anyhow::Result;
use serde::Deserialize;
use std::{collections::BTreeSet, fs, path::Path};
use teloxide::types::{ChatId, UserId};

#[derive(Deserialize)]
pub struct Config {
    pub token: String,
    pub super_admins: BTreeSet<UserId>,
    pub support_chat: ChatId,
}
impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        Ok(toml::from_str(fs::read_to_string(path)?.as_str())?)
    }
}
