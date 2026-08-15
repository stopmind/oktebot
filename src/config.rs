use std::fs::File;
use std::io::Read;
use std::path::Path;
use serde::Deserialize;
use teloxide::types::{ChatId, UserId};
use anyhow::Result;

#[derive(Deserialize)]
pub struct Config {
    pub token: String,
    pub super_admins: Vec<UserId>,
    pub admin_chat: ChatId
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let mut string = String::new();
        File::open(path)?
            .read_to_string(&mut string)?;
        Ok(toml::from_str(string.as_str())?)
    }
}