mod config;
mod utils;
mod support;
mod command;
mod scheme;
mod oknoid;
mod session;

use std::sync::Arc;
use log::{info, LevelFilter};
use teloxide::prelude::*;
use teloxide::dispatching::dialogue::InMemStorage;
use crate::config::Config;
use crate::oknoid::IdDb;
use crate::scheme::scheme;
use crate::session::SessionState;
use crate::support::*;

#[tokio::main]
async fn main() {
    stop_log::init(None, LevelFilter::Info);
    info!("Starting bot...");

    let config = Config::read("config.toml").unwrap();
    let db = IdDb::open("id.db").await.unwrap();

    let bot = Bot::new(config.token.clone());

    Dispatcher::builder(bot, scheme())
        .dependencies(dptree::deps![
            InMemStorage::<SessionState>::new(),
            Arc::new(config),
            db
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
