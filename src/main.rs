mod bot;
mod config;
mod oknoid;

use crate::{
    bot::{BotContext, index, index::Limiter, scheme::scheme, session::Sessions, set_commands},
    config::Config,
    oknoid::OknoId,
};
use anyhow::anyhow;
use log::{LevelFilter, error, info};
use std::{env, fs, sync::Arc};
use teloxide::prelude::*;

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

    let db = Arc::new(OknoId::open(config.clone()).await?);
    let bot = Bot::new(config.token.clone());

    tokio::spawn({
        let bot = bot.clone();
        async move {
            if let Err(err) = set_commands(&bot).await {
                error!("{}", err);
            }
        }
    });

    tokio::spawn({
        let bot = bot.clone();
        let config = config.clone();
        let db = db.clone();
        index::check_games_index_worker(bot, config, db)
    });

    let limiter = Arc::new(Limiter::default());

    tokio::spawn(limiter.clone().worker());

    let me = bot.get_me().await?;

    let router = Arc::new(scheme());
    router
        .handle_updates(Arc::new(BotContext {
            bot,
            config,
            db,
            me,
            sessions: Sessions::default(),
            limiter,
        }))
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
