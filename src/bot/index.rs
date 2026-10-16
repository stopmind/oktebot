use crate::{
    bot::{
        args::get_args,
        scheme::{INDEX_CALLBACK, MENU_CALLBACK},
        session::{Session, SessionState},
        utils,
        utils::{UtilError, cancel_button, menu_button, try_delete_origin},
    },
    config::Config,
    oknoid::OknoId,
};
use anyhow::{Result, anyhow};
use log::{error, info};
use reqwest::{Client, IntoUrl, Method, StatusCode};
use scraper::{Html, Selector};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use teloxide::{
    Bot,
    dispatching::dialogue::GetChatId,
    payloads::{SendMessageSetters, SendPhotoSetters},
    requests::Requester,
    types::{
        CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, Message,
        ParseMode, ReplyMarkup, UserId,
    },
};
use tokio::time::sleep;
use url::Url;

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

async fn get_data<D: DeserializeOwned>(token: &str, url: impl IntoUrl) -> Result<D> {
    let data = Client::new()
        .request(Method::GET, url)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(data)
}

async fn get_game_data_by_url(token: &str, url: Url) -> Result<GameDataByUrl> {
    let url = data_url_from_game_url(url)?;

    let data = get_data(token, url).await?;
    Ok(data)
}

async fn get_game_data_by_id(token: &str, game_id: i32) -> Result<GameDataById> {
    #[derive(Deserialize)]
    struct GameDataResponse {
        game: GameDataById,
    }

    let data: GameDataResponse =
        get_data(token, format!("https://api.itch.io/games/{game_id}")).await?;
    Ok(data.game)
}

async fn index(
    db: &OknoId,
    bot: &Bot,
    config: &Config,
    limiter: &Limiter,
    chat_id: ChatId,
    user_id: UserId,
    url: &str,
    add_buttons: bool,
) -> Result<bool> {
    if !limiter.check(user_id) {
        bot.send_message(chat_id, "Между запросами должно пройти 2 минуты.")
            .reply_markup(InlineKeyboardMarkup::new(
                add_buttons.then(|| [menu_button()]),
            ))
            .await?;
        return Ok(true);
    }

    if let Ok(url) = Url::parse(url)
        && let Some(domain) = url.domain()
        && domain.ends_with("itch.io")
    {
        limiter.write_use(user_id);
        let data = match get_game_data_by_url(config.itch_token.as_str(), url).await {
            Ok(data) => data,
            Err(err) => {
                error!("error get game data: {err:?}");

                let msg = match err
                    .downcast_ref::<reqwest::Error>()
                    .and_then(|err| err.status())
                {
                    Some(StatusCode::NOT_FOUND) => "Игра не найдена.",
                    Some(StatusCode::TOO_MANY_REQUESTS) => {
                        "Слишком много запросов, попробуйте позже"
                    }
                    _ => "Не удалось получить информацию о игре с серверов itch.io",
                };

                bot.send_message(chat_id, msg)
                    .reply_markup(InlineKeyboardMarkup::new(
                        add_buttons.then(|| [menu_button()]),
                    ))
                    .await?;

                return Ok(true);
            }
        };

        let is_indexed = check_game_index(data.id, data.title.as_str()).await?;
        let msg = if is_indexed {
            "Игра уже проиндексирована!"
        } else {
            let changed = db.add_index_sub(user_id, data.id).await?;
            if changed {
                "Когда игра будет проиндексирована вы получите уведомление."
            } else {
                "Вы уже подписаны на индексацию этой игры."
            }
        };

        let mut send = bot.send_message(chat_id, msg);
        if add_buttons {
            send.reply_markup = Some(ReplyMarkup::inline_kb([[menu_button()]]))
        }
        send.await?;

        Ok(true)
    } else {
        let mut send = bot.send_message(
            chat_id,
            "Вы должны указать ссылку на игру с itch.io, начинающаяся с https://",
        );

        if add_buttons {
            send.reply_markup = Some(ReplyMarkup::inline_kb([[utils::cancel_button()]]))
        }

        send.await?;
        Ok(false)
    }
}

pub async fn on_index_url_message(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    limiter: Arc<Limiter>,
    session: Session,
    message: Message,
) -> Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    let finished = index(
        &db,
        &bot,
        &config,
        &limiter,
        message.chat.id,
        user.id,
        message.text().unwrap_or(""),
        true,
    )
    .await?;

    if finished {
        session.reset().await?;
    }

    Ok(())
}

async fn start_index_dialogue(bot: &Bot, limiter: &Limiter, chat_id: ChatId, user_id: UserId, session: &Session) -> Result<()> {
    if !limiter.check(user_id) {
        bot.send_message(chat_id, "Между запросами должно пройти 2 минуты.")
            .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
            .await?;
        return Ok(());
    }

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
    config: Arc<Config>,
    limiter: Arc<Limiter>,
    session: Session,
) -> Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    let url = get_args(&message);
    if url.is_empty() {
        start_index_dialogue(&bot, &limiter, message.chat.id, user.id, &session).await?;
    } else {
        index(
            &db,
            &bot,
            &config,
            &limiter,
            message.chat.id,
            user.id,
            url,
            false,
        )
        .await?;
    }

    Ok(())
}

pub async fn on_index_callback(bot: Bot, limiter: Arc<Limiter>, callback: CallbackQuery, session: Session) -> Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;
    start_index_dialogue(&bot, &limiter, chat_id, callback.from.id, &session).await?;
    bot.answer_callback_query(callback.id).await?;

    Ok(())
}

pub async fn on_index_help_callback(
    bot: Bot,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    bot.send_photo(chat_id, InputFile::url(config.banners.index.clone()))
        .caption("<b>OknoIndexCheck</b> — удобный <b>инструмент</b>, чтобы разобраться в индексации ваших игр на itch io.

Отправляйте itch.io <b>ссылку</b>, чтобы проверить <b>проиндексирована</b> ли ваша игра. Если <b>нет</b>, то бот уведомит вас позже, когда индексация игры произойдет

<i>Подробнее об индексации</i> &gt; <a href=\"https://t.me/oknogmdv/2706\">пост</a>")
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new([
            [InlineKeyboardButton::callback("Проверить", INDEX_CALLBACK)],
            [menu_button()]
        ]))
        .await?;

    try_delete_origin(&bot, &callback).await?;
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

const TIMEOUT: Duration = Duration::from_mins(2);

#[derive(Default)]
pub struct Limiter {
    data: Mutex<BTreeMap<UserId, Instant>>,
}

impl Limiter {
    /// true if timeout finished
    fn check(&self, user_id: UserId) -> bool {
        self.data
            .lock()
            .unwrap()
            .get(&user_id)
            .is_none_or(|data| data.elapsed() > TIMEOUT)
    }

    fn write_use(&self, user_id: UserId) {
        self.data.lock().unwrap().insert(user_id, Instant::now());
    }

    pub async fn worker(self: Arc<Self>) {
        sleep(Duration::from_mins(15)).await;
        self.data
            .lock()
            .unwrap()
            .retain(|_, data| data.elapsed() < TIMEOUT);
    }
}
