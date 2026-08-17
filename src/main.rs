mod config;
mod oknoid;
mod bot;

use std::sync::Arc;
use log::{info, LevelFilter};
use teloxide::prelude::*;
use teloxide::dispatching::dialogue::InMemStorage;
use crate::config::Config;
use crate::oknoid::OknoId;
use bot::scheme::scheme;
use bot::session::SessionState;
use bot::support::*;

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
