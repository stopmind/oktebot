mod bot;
mod config;
mod oknoid;

use crate::{config::Config, oknoid::OknoId};
use bot::{scheme::scheme, session::SessionState};
use log::{LevelFilter, info};
use std::sync::Arc;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

#[tokio::main]
async fn main() {
    stop_log::init(None, LevelFilter::Info);
    info!("Starting bot...");

    let config = Arc::new(Config::read("config.toml").unwrap());
    let db = OknoId::open("id.db", config.clone()).await.unwrap();

    let bot = Bot::new(config.token.clone());

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
}
