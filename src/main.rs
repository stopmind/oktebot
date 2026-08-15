mod config;
mod utils;
mod support;
mod command;
mod scheme;

use std::sync::Arc;
use log::{info, LevelFilter};
use teloxide::prelude::*;
use teloxide::dispatching::dialogue::InMemStorage;
use crate::config::Config;
use crate::scheme::scheme;
use crate::support::*;

#[tokio::main]
async fn main() {
    stop_log::init(None, LevelFilter::Info);
    info!("Starting bot...");

    let config = Config::read("config.toml").unwrap();

    let bot = Bot::new(config.token.clone());

    Dispatcher::builder(bot, scheme())
        .dependencies(dptree::deps![InMemStorage::<SupportState>::new(), Arc::new(config)])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
