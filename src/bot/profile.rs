use crate::{
    bot::{
        args::{Mention, get_args},
        invalid_usage_message, main_menu,
        scheme::{
            BIO_CALLBACK, CANCEL_CALLBACK, MENU_CALLBACK, PROFILE_CALLBACK_PREFIX,
            TOP_CALLBACK_PREFIX,
        },
        session::{Session, SessionState},
        utils::{
            UtilError, check_private, check_user_privileges, get_callback_chat, get_id_username,
            menu_button, resolve_mention,
        },
    },
    config::Config,
    oknoid::{IdError, OknoId, Role, UserInfo},
    parser,
};
use log::error;
use std::{fmt::Write, iter, sync::Arc};
use teloxide::{
    Bot,
    dispatching::dialogue::GetChatId,
    payloads::{SendMessageSetters, SendPhotoSetters},
    prelude::{CallbackQuery, ChatId, Message, Requester, UserId},
    types::{
        Chat, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, InputFile,
        ParseMode,
    },
};

pub async fn usernames_inspect(message: Message, db: Arc<OknoId>) {
    if let Ok((id, username)) = get_id_username(message.from.as_ref())
        && let Err(err) = db.update_username(id, username.to_owned()).await
    {
        error!("Error while updating username: {:?}", err);
    }
}

pub async fn on_start(
    bot: Bot,
    config: Arc<Config>,
    message: Message,
    db: Arc<OknoId>,
) -> anyhow::Result<()> {
    let (id, username) = get_id_username(message.from.as_ref())?;

    if let Err(error) = db
        .register_user(id, username.to_owned(), UserInfo::default())
        .await
        && !matches!(error, IdError::UserExists(..))
    {
        error!("Error registering user: {:?}", error);
    };

    main_menu(&bot, &config, &message.chat).await
}

async fn bio(bot: &Bot, session: &Session, chat: &Chat) -> anyhow::Result<()> {
    check_private(bot, chat).await?;
    session.update(SessionState::WaitBioMessage).await?;

    bot.send_message(
        chat.id,
        "Отправьте описание для профиля следующим сообщением.",
    )
    .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
        "Отмена",
        InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
    )]]))
    .await?;
    Ok(())
}

pub async fn on_bio_command(bot: Bot, session: Session, message: Message) -> anyhow::Result<()> {
    bio(&bot, &session, &message.chat).await?;
    Ok(())
}

pub async fn on_bio_callback(
    bot: Bot,
    session: Session,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat = get_callback_chat(&callback)?;
    bio(&bot, &session, chat).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_bio_message(
    bot: Bot,
    session: Session,
    message: Message,
    db: Arc<OknoId>,
) -> anyhow::Result<()> {
    let Some(text) = message.text() else {
        bot.send_message(message.chat.id, "Отправьте сообщение с текстом!")
            .await?;
        return Ok(());
    };

    if text.trim().is_empty() {
        bot.send_message(message.chat.id, "Описание не может быть пустым! >:(")
            .await?;
        return Ok(());
    }

    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    session.exit().await?;

    let result = db.set_bio(user.id, Some(text)).await;
    if let Err(error) = result {
        bot.send_message(message.chat.id, "Не удалось изменить описание.")
            .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
            .await?;
        Err(error.into())
    } else {
        bot.send_message(message.chat.id, "Описание профиля обновлено.")
            .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
            .await?;
        Ok(())
    }
}

async fn send_profile(
    bot: &Bot,
    db: &OknoId,
    chat_id: ChatId,
    user_id: UserId,
    username: &str,
    is_me: bool,
    menu_button: bool,
) -> anyhow::Result<()> {
    let info = db.get_user_info(user_id).await?;

    let text = if let Some(bio) = info.bio.as_ref() {
        format!(
            "> Username: {username}\n\
            > Bio: {bio}\n\
            > Reputation: {} ⚡",
            info.reputation,
        )
    } else {
        format!(
            "> Username: {username}\n\
            > Reputation: {} ⚡",
            info.reputation,
        )
    };

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new([iter::chain(
            is_me.then(|| {
                InlineKeyboardButton::callback(
                    if info.bio.is_some() {
                        "Изменить bio"
                    } else {
                        "Добавить bio"
                    },
                    BIO_CALLBACK,
                )
            }),
            menu_button.then(|| InlineKeyboardButton::callback("В меню", MENU_CALLBACK)),
        )]))
        .await?;
    Ok(())
}

pub async fn on_info_command(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let info = match mention {
            Mention::Username(username) => (db.resolve_username(&username), Some(username)),
            Mention::UserId(id) => (Some(id), db.get_username(id)),
        };

        if let (Some(id), Some(username)) = info {
            send_profile(
                &bot,
                &db,
                message.chat.id,
                id,
                username.as_ref(),
                false,
                false,
            )
            .await?;
        } else {
            bot.send_message(message.chat.id, "Пользователь не найден")
                .await?;
        }
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_me_command(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let (id, username) = get_id_username(message.from.as_ref())?;

    send_profile(&bot, &db, message.chat.id, id, username, true, false).await
}

pub async fn on_me_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    let username = callback
        .from
        .username
        .as_deref()
        .ok_or(UtilError::FailedGetUser)?;

    send_profile(&bot, &db, chat_id, callback.from.id, username, true, true).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_profile_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    let callback_data = callback
        .data
        .as_ref()
        .ok_or(UtilError::FailedParseCallbackData)?;

    let user_id = callback_data[PROFILE_CALLBACK_PREFIX.len()..]
        .parse()
        .map_err(|_| UtilError::FailedParseCallbackData)
        .map(UserId)?;

    let username = db.get_username(user_id).ok_or(UtilError::FailedGetUser)?;

    send_profile(&bot, &db, chat_id, user_id, username.as_str(), false, false).await?;
    bot.answer_callback_query(callback.id).await?;

    Ok(())
}

pub async fn on_add_admin_command(
    bot: Bot,
    message: Message,
    db: Arc<OknoId>,
) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let id = resolve_mention(&bot, &db, message.chat.id, &mention).await?;

        if db.give_role(id, Role::Admin).await? {
            bot.send_message(message.chat.id, "Пользователь назначен админом.")
                .await?;
        } else {
            bot.send_message(message.chat.id, "Пользователь уже является админом.")
                .await?;
        }
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }
    Ok(())
}

pub async fn on_del_admin(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let id = resolve_mention(&bot, &db, message.chat.id, &mention).await?;

        if db.take_role(id, Role::Admin).await? {
            bot.send_message(message.chat.id, "Пользователь более не является админом.")
                .await?;
        } else {
            bot.send_message(message.chat.id, "Пользователь не админ.")
                .await?;
        }
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_change_rep(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    check_user_privileges(&bot, &db, user.id, message.chat.id).await?;

    let args = get_args(&message);
    if let Some((mention, value)) = parser![Mention, i64](args) {
        let target_id = resolve_mention(&bot, &db, message.chat.id, &mention).await?;
        let new_rep = db.add_reputation(target_id, value).await?;

        bot.send_message(message.chat.id, format!("Обновленная репутация: {new_rep}"))
            .await?;
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

async fn top(
    bot: &Bot,
    db: &OknoId,
    config: &Config,
    chat_id: ChatId,
    page: u32,
) -> anyhow::Result<()> {
    const PAGE_SIZE: u32 = 20;
    let top_data = db.get_top(page * PAGE_SIZE, PAGE_SIZE).await?;
    let users_count = db.users_count().await?;
    let pages_count = users_count.div_ceil(PAGE_SIZE);

    let mut text = format!(
        "<b>Таблица репутации OknoMembers:</b> ({users_count} пользователей, страница {}/{pages_count})\n",
        page + 1
    );
    for (i, (_, rep, username)) in top_data.into_iter().enumerate() {
        match (i, page) {
            (0, 0) => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388614717164005740\">🪷</tg-emoji> <b>{username}</b> - {rep} rep."
            )?,
            (1, 0) => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388967879439852799\">🌸</tg-emoji> <b>{username}</b> - {rep} rep."
            )?,
            (2, 0) => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388956849963837711\">🌸</tg-emoji> <b>{username}</b> - {rep} rep."
            )?,
            _ => writeln!(&mut text, "&gt; <b>{username}</b> - {rep} rep.")?,
        }
    }

    let markup = InlineKeyboardMarkup::new(
        [
            (page > 0).then(|| {
                [InlineKeyboardButton::callback(
                    "< Предыдущая страница",
                    format!("{TOP_CALLBACK_PREFIX}{}", page - 1),
                )]
            }),
            (page + 1 < pages_count).then(|| {
                [InlineKeyboardButton::callback(
                    "Следующая страница >",
                    format!("{TOP_CALLBACK_PREFIX}{}", page + 1),
                )]
            }),
            Some([menu_button()]),
        ]
        .into_iter()
        .flatten(),
    );

    bot.send_photo(chat_id, InputFile::file_id(config.banners.top.clone()))
        .caption(text)
        .parse_mode(ParseMode::Html)
        .reply_markup(markup)
        .await?;

    Ok(())
}

pub async fn on_top_command(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    message: Message,
) -> anyhow::Result<()> {
    top(&bot, &db, &config, message.chat.id, 0).await
}

pub async fn on_top_callback(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;
    let data = callback.data.as_deref().ok_or(UtilError::NoCallbackData)?;

    let page = data[TOP_CALLBACK_PREFIX.len()..].parse()?;
    top(&bot, &db, &config, chat_id, page).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}
