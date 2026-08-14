use log::LevelFilter;
use teloxide::prelude::*;

#[tokio::main]
async fn main() {
    stop_log::init(None, LevelFilter::Info);
    log::info!("Starting dialogue bot...");

    let bot = Bot::new("8972146142:AAE-9lsvhoEaqiD35qP-uGwURXBwXJnomgE");

    Dispatcher::<_, (), _>::builder(
        bot,
        Update::filter_message()
    )
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
}
