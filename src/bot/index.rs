use crate::{
    bot::{args::get_args, session::Session, utils::UtilError},
    config::Config,
    oknoid::OknoId,
};
use anyhow::{anyhow, Result};
use log::{error, info};
use reqwest::{Client, Method};
use scraper::{Html, Selector};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use teloxide::{
    payloads::SendMessageSetters,
    requests::Requester,
    types::{CallbackQuery, ChatId, Message, ParseMode, UserId},
    Bot,
};
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::types::{InlineKeyboardMarkup, ReplyMarkup};
use tokio::time::sleep;
use url::Url;
use crate::bot::session::SessionState;
use crate::bot::utils;
use crate::bot::utils::cancel_button;

async fn check_game_index(id: i32, title: &str) -> Result<bool> {
    let response = reqwest::get(Url::parse_with_params(
        "https://itch.io/search",
        &[("q", title)],
    )?)
    .await?;
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

    let data = reqwest::get(url).await?.error_for_status()?.json().await?;

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

async fn index(db: &OknoId, bot: &Bot, chat_id: ChatId, user_id: UserId, url: &str, cancel_button: bool) -> Result<bool> {
    if let Ok(url) = Url::parse(url)
        && let Some(domain) = url.domain()
        && domain.ends_with("itch.io")
    {
        let data = match get_game_data_by_url(url).await {
            Ok(data) => data,
            Err(err) => {
                error!("error get game data: {err:?}");

                if let Some(req_err) = err.downcast_ref::<reqwest::Error>()
                    && req_err.status() == Some(reqwest::StatusCode::NOT_FOUND)
                {
                    bot.send_message(chat_id, "Игра не найдена.")
                        .await?;
                } else {
                    bot.send_message(chat_id, "Не удалось получить информацию о игре.")
                        .await?;
                }

                return Ok(true);
            }
        };

        let is_indexed = check_game_index(data.id, data.title.as_str()).await?;
        if is_indexed {
            bot.send_message(chat_id, "Игра уже проиндексирована!")
                .await?;
        } else {
            let changed = db.add_index_sub(user_id, data.id).await?;
            let msg = if changed {
                "Когда игра будет проиндексирована вы получите уведомление."
            } else {
                "Вы уже подписаны на индексацию этой игры."
            };
            bot.send_message(chat_id, msg).await?;
        }

        Ok(true)
    } else {
        let mut send = bot.send_message(
            chat_id,
            "Вы должны указать ссылку на игру с itch.io, начинающаяся с https://",
        );

        if cancel_button {
            send.reply_markup = Some(ReplyMarkup::inline_kb([[utils::cancel_button()]]))
        }

        send.await?;
        Ok(false)
    }
}

pub async fn on_index_url_message(bot: Bot, db: Arc<OknoId>, session: Session, message: Message) -> Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    let finished = index(&db, &bot, message.chat.id, user.id, message.text().unwrap_or(""), true).await?;

    if finished {
        session.reset().await?;
    }

    Ok(())
}

async fn start_index_dialogue(bot: &Bot, chat_id: ChatId, session: &Session) -> Result<()> {
    session.update(SessionState::WaitIndexUrl).await?;
    bot.send_message(chat_id, "Введите ссылку на игру с itch.io")
        .reply_markup(InlineKeyboardMarkup::new([[cancel_button()]]))
        .await?;
    Ok(())
}

pub async fn on_index_command(
    message: Message,
    db: Arc<OknoId>,
    bot: Bot,
    session: Session,
) -> Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    let url = get_args(&message);
    if url.is_empty() {
        start_index_dialogue(&bot, message.chat.id, &session).await?;
    } else {
        index(&db, &bot, message.chat.id, user.id, url, false).await?;
    }

    Ok(())
}

pub async fn on_index_callback(
    bot: Bot,
    callback: CallbackQuery,
    session: Session,
) -> Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;
    start_index_dialogue(&bot, chat_id, &session).await?;
    bot.answer_callback_query(callback.id).await?;

    Ok(())
}

async fn check_games(bot: &Bot, db: &OknoId, itch_token: &str) -> Result<()> {
    let notify_user = async |user_id, url: &Url, title: &str| -> Result<()> {
        let msg = format!("Игра <b>{title}</b> была проиндексирована: {}", url);
        bot.send_message(user_id, msg)
            .parse_mode(ParseMode::Html)
            .await?;
        Ok(())
    };

    info!("start games index check");

    let subs = db.get_all_index_subs().await?;
    let mut indexed_games: BTreeMap<_, (Url, String)> = BTreeMap::new();

    for (user_id, game_id) in subs {
        if let Some((url, title)) = indexed_games.get(&game_id) {
            notify_user(user_id, url, title.as_str()).await?;
        } else {
            let GameDataById { title, url } = get_game_data_by_id(itch_token, game_id).await?;
            let is_indexed = check_game_index(game_id, title.as_str()).await?;

            if is_indexed {
                info!("game indexed: {game_id}");
                notify_user(user_id, &url, title.as_str()).await?;
                indexed_games.insert(game_id, (url, title));
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
