use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use teloxide::types::{ChatId, UserId};

#[derive(Deserialize)]
pub struct Config {
    pub token: String,
    pub super_admins: BTreeSet<UserId>,
    pub admin_chat: ChatId
}
impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        Ok(toml::from_str(
            fs::read_to_string(path)?.as_str()
        )?)
    }
}