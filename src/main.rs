mod api;
mod bot;
mod config;
pub mod login;
mod oknoid;

use crate::{api::serve_api, bot::start_bot, config::Config, login::Login, oknoid::OknoId};
use anyhow::anyhow;
use log::{LevelFilter, error};
use std::{env, fs, sync::Arc};

async fn start() -> anyhow::Result<()> {
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
    let login = Arc::new(Login::default());

    tokio::spawn(login.clone().clear_token_worker());

    tokio::spawn({
        let db = db.clone();
        let login = login.clone();

        async {
            if let Err(err) = serve_api(db, login).await {
                error!("api server error: {}", err);
            }
        }
    });
    start_bot(config, db, login).await
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    stop_log::init(None, LevelFilter::Info);
    if let Err(e) = start().await {
        error!("{}", e);
    }
}
