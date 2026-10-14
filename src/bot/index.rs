use crate::{
    bot::{args::get_args, utils::UtilError},
    config::Config,
    oknoid::OknoId,
};
use anyhow::{Result, anyhow};
use log::{error, info};
use reqwest::{Client, Method};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::Duration,
};
use scraper::{Html, Selector};
use teloxide::{Bot, requests::Requester, types::Message};
use tokio::time::sleep;
use url::Url;

async fn check_game_index(id: i32, title: &str) -> Result<bool> {
    let response = reqwest::get(Url::parse_with_params("https://itch.io/search", &[("q", title)])?).await?;
    let document = Html::parse_document(&response.text().await?);
    let id_str = id.to_string();

    let selector = Selector::parse(".game_cell").unwrap();

    info!("Title: {title} Id: {id_str}");

    for element in document.select(&selector) {
        info!(" Found: {}", element.attr("data-game_id").unwrap_or("NONE"));
        if element.attr("data-game_id") == Some(id_str.as_str()) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn data_url_from_game_url(mut game_url: Url) -> Result<Url> {
    game_url
        .path_segments_mut()
        .map_err(|_| anyhow!("url crate sucks"))?
        .push("data.json");

    Ok(game_url)
}

#[derive(Deserialize)]
struct GameDataByUrl {
    id: i32,
    title: String,
}

#[derive(Deserialize)]
struct GameDataById {
    title: String,
    url: Url,
}

async fn get_game_data_by_url(url: Url) -> Result<GameDataByUrl> {
    let url = data_url_from_game_url(url)?;

    let data = reqwest::get(url).await?.json().await?;

    Ok(data)
}

async fn get_game_data_by_id(token: &str, game_id: i32) -> Result<GameDataById> {
    #[derive(Deserialize)]
    struct GameDataResponse {
        game: GameDataById,
    }

    let data: GameDataResponse = Client::new()
        .request(Method::GET, format!("https://api.itch.io/games/{game_id}"))
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    Ok(data.game)
}

pub async fn on_index_command(
    message: Message,
    db: Arc<OknoId>,
    bot: Bot
) -> Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    if let Ok(url) = Url::parse(get_args(&message))
        && let Some(domain) = url.domain()
        && domain.ends_with("itch.io")
    {
        let data = match get_game_data_by_url(url).await {
            Ok(data) => data,
            Err(err) => {
                error!("error get game data: {err:?}");
                bot.send_message(message.chat.id, "Не удалось получить информацию о игре.")
                    .await?;
                return Ok(());
            }
        };

        let is_indexed =
            check_game_index(data.id, data.title.as_str()).await?;
        if is_indexed {
            bot.send_message(message.chat.id, "Игра уже проиндексирована!")
                .await?;
        } else {
            let changed = db.add_index_sub(user.id, data.id).await?;
            let msg = if changed {
                "Когда игра будет проиндексирована вы получите уведомление."
            } else {
                "Вы уже подписаны на индексацию этой игры."
            };
            bot.send_message(message.chat.id, msg).await?;
        }

        Ok(())
    } else {
        bot.send_message(
            message.chat.id,
            "Аргументом должна быть ссылка на игру с itch.io, начинающаяся с https://",
        )
        .await?;
        Ok(())
    }
}

async fn check_games(bot: &Bot, db: &OknoId, itch_token: &str) -> Result<()> {
    let notify_user = async |user_id, url: &Url| -> Result<()> {
        let msg = format!("Игры была проиндексирована: {}", url);
        bot.send_message(user_id, msg).await?;
        Ok(())
    };

    info!("start games index check");

    let subs = db.get_all_index_subs().await?;
    let mut indexed_games = BTreeMap::new();

    for (user_id, game_id) in subs {
        if let Some(url) = indexed_games.get(&game_id) {
            notify_user(user_id, url).await?;
        } else {
            let data = get_game_data_by_id(itch_token, game_id).await?;
            let is_indexed = check_game_index(game_id, data.title.as_str()).await?;

            if is_indexed {
                info!("game indexed: {game_id}");
                notify_user(user_id, &data.url).await?;
                indexed_games.insert(game_id, data.url);
                db.remove_index_subs(game_id).await?;
            }
        }
    }

    Ok(())
}

pub async fn check_games_index_worker(bot: Bot, config: Arc<Config>, db: Arc<OknoId>) {
    loop {
        if let Err(err) = check_games(&bot, &db, &config.itch_token).await {
            error!("error check games index: {err}");
        }
        sleep(Duration::from_hours(1)).await;
    }
}
