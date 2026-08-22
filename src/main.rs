mod bot;
mod config;
mod oknoid;

use crate::{bot::set_commands, config::Config, oknoid::OknoId};
use anyhow::anyhow;
use bot::{scheme::scheme, session::SessionState};
use log::{LevelFilter, error, info};
use std::{env, fs, sync::Arc};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

async fn start() -> anyhow::Result<()> {
    info!("Starting bot...");

    let config_path = env::var("OKTEBOT_CONFIG");
    let config_path = config_path
        .as_ref()
        .map(String::as_str)
        .unwrap_or("/etc/oktebot.toml");

    let config = Arc::new(Config::read(config_path)?);

    if !config.storage.exists() {
        fs::create_dir_all(&config.storage)
            .map_err(|e| anyhow!("failed to create storage directory: {}", e))?;
    }

    let db = OknoId::open(config.clone()).await?;
    let bot = Bot::new(config.token.clone());

    set_commands(&bot).await?;

    Dispatcher::builder(bot, scheme())
        .dependencies(dptree::deps![
            InMemStorage::<SessionState>::new(),
            config,
            Arc::new(db)
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

#[tokio::main]
async fn main() {
    stop_log::init(None, LevelFilter::Info);
    if let Err(e) = start().await {
        error!("{}", e);
    }
}
