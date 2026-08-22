use crate::{
    bot::{
        args::{Mention, get_args},
        invalid_usage_message, main_menu,
        scheme::{CANCEL_CALLBACK, PROFILE_CALLBACK_PREFIX},
        session::{Session, SessionState},
    },
    oknoid::{IdError, OknoId, Role, UserInfo},
    parser,
};
use anyhow::{anyhow, bail};
use itertools::Itertools;
use log::error;
use std::{fmt::Write, sync::Arc};
use teloxide::{
    Bot,
    dispatching::dialogue::GetChatId,
    payloads::SendMessageSetters,
    prelude::{CallbackQuery, ChatId, Message, Requester, UserId},
    types::{
        ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, ParseMode,
        User,
    },
};

pub async fn usernames_inspect(message: Message, db: Arc<OknoId>) {
    if let Some(User {
        id,
        username: Some(username),
        ..
    }) = message.from.as_ref()
        && let Err(err) = db.update_username(*id, username.clone()).await
    {
        error!("Error while updating username: {:?}", err);
    }
}

pub async fn on_start(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let Some(User {
        id,
        username: Some(username),
        ..
    }) = message.from.as_ref()
    else {
        bail!("failed get user or username");
    };

    if let Err(error) = db
        .register_user(*id, username.clone(), UserInfo::default())
        .await
        && !matches!(error, IdError::UserExists(..))
    {
        error!("Error registering user: {:?}", error);
    };

    main_menu(&bot, message.chat.id).await
}

pub async fn on_bio(bot: Bot, session: Session, message: Message) -> anyhow::Result<()> {
    if matches!(message.chat.kind, ChatKind::Private(..)) {
        session.update(SessionState::WaitBioMessage).await?;

        bot.send_message(
            message.chat.id,
            "Отправьте описание для профиля следующим сообщением.",
        )
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "Отмена",
            InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
        )]]))
        .await?;
    } else {
        bot.send_message(
            message.chat.id,
            "Команда может быть использована только в личных сообщениях.",
        )
        .await?;
    }

    Ok(())
}

pub async fn on_bio_callback(
    bot: Bot,
    session: Session,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let Some(message) = callback.regular_message() else {
        bail!("callback message not found");
    };

    if matches!(message.chat.kind, ChatKind::Private(..)) {
        session.update(SessionState::WaitBioMessage).await?;

        bot.send_message(
            message.chat.id,
            "Отправьте описание для профиля сообщением.",
        )
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "Отмена",
            InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
        )]]))
        .await?;
    } else {
        bot.send_message(
            message.chat.id,
            "Команда может быть использована только в личных сообщениях.",
        )
        .await?;
    }

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

    let user = message
        .from
        .as_ref()
        .ok_or(anyhow!("Failed to retrieve user"))?;

    session.exit().await?;

    let result = db.set_bio(user.id, Some(text)).await;
    if let Err(error) = result {
        bot.send_message(message.chat.id, "Не удалось изменить описание.")
            .await?;
        Err(error.into())
    } else {
        bot.send_message(message.chat.id, "Описание профиля обновлено.")
            .await?;
        Ok(())
    }
}

pub async fn on_bio_cancel(bot: Bot, query: CallbackQuery, session: Session) -> anyhow::Result<()> {
    session.exit().await?;
    bot.send_message(session.chat_id(), "Отменено.").await?;
    bot.answer_callback_query(query.id).await?;
    Ok(())
}

async fn send_profile(
    bot: &Bot,
    db: &OknoId,
    chat_id: ChatId,
    user_id: UserId,
    username: &str,
) -> anyhow::Result<()> {
    let info = db.get_user_info(user_id).await?;
    let roles_string = info.roles.iter().join(", ");
    let text = format!(
        "Пользователь: @{username}\n\
        Репутация: {}.\n\
        Роли: {}.\n\
        Описание: {}.",
        info.reputation,
        if info.roles.is_empty() {
            "нет"
        } else {
            roles_string.as_str()
        },
        info.bio.as_deref().unwrap_or("нет")
    );

    bot.send_message(chat_id, text).await?;
    Ok(())
}

pub async fn on_info(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let info = match mention {
            Mention::Username(username) => (db.resolve_username(&username), Some(username)),
            Mention::UserId(id) => (Some(id), db.get_username(id)),
        };

        if let (Some(id), Some(username)) = info {
            send_profile(&bot, &db, message.chat.id, id, username.as_ref()).await?;
        } else {
            bot.send_message(message.chat.id, "Пользователь не найден")
                .await?;
        }
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_me(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let Some(User {
        id,
        username: Some(username),
        ..
    }) = message.from.as_ref()
    else {
        bail!("failed get user or username");
    };

    send_profile(&bot, &db, message.chat.id, *id, username.as_str()).await
}

pub async fn me_callback(bot: Bot, db: Arc<OknoId>, callback: CallbackQuery) -> anyhow::Result<()> {
    let chat_id = callback
        .chat_id()
        .ok_or_else(|| anyhow!("Failed to get callback chat id"))?;

    let username = callback
        .from
        .username
        .as_deref()
        .ok_or_else(|| anyhow!("Failed to get username"))?;

    send_profile(&bot, &db, chat_id, callback.from.id, username).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_profile_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let message = callback
        .regular_message()
        .ok_or_else(|| anyhow!("callback message not found"))?;

    let callback_data = callback
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("callback not found"))?;

    let id = callback_data[PROFILE_CALLBACK_PREFIX.len()..]
        .parse()
        .map_err(|_| anyhow!("failed to parse callback data: {}", callback_data))
        .map(UserId)?;

    let username = db
        .get_username(id)
        .ok_or_else(|| anyhow!("username not found, user_id: {id}"))?;

    send_profile(&bot, &db, message.chat.id, id, username.as_str()).await?;
    bot.answer_callback_query(callback.id).await?;

    Ok(())
}

pub async fn add_admin(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let Some(id) = mention.resolve(&db) else {
            bot.send_message(message.chat.id, "Пользователь не найден!")
                .await?;
            return Ok(());
        };

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

pub async fn del_admin(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let Some(id) = mention.resolve(&db) else {
            bot.send_message(message.chat.id, "Пользователь не найден!")
                .await?;
            return Ok(());
        };

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

pub async fn change_rep(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let Some(User { id: user_id, .. }) = message.from else {
        bail!("Failed get user id");
    };

    if !db.check_user_privileges(user_id).await? {
        bot.send_message(message.chat.id, "У вас недостаточно прав")
            .await?;
        return Ok(());
    }

    let args = get_args(&message);
    if let Some((mention, value)) = parser![Mention, i64](args) {
        let Some(target_id) = mention.resolve(&db) else {
            bot.send_message(message.chat.id, "Неизвестный пользователь")
                .await?;
            return Ok(());
        };

        let new_rep = db.add_reputation(target_id, value).await?;

        bot.send_message(message.chat.id, format!("Обновленная репутация: {new_rep}"))
            .await?;
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_top(bot: Bot, db: Arc<OknoId>, message: Message) -> anyhow::Result<()> {
    let top_data = db.get_top(0, 20).await?;

    let mut text = "<b>Таблица репутации OknoMembers:</b>\n".to_string();
    for (i, (_, rep, username)) in top_data.into_iter().enumerate() {
        match i {
            0 => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388614717164005740\">🪷</tg-emoji> <b>{username}</b> - {rep} rep."
            )?,
            1 => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388967879439852799\">🌸</tg-emoji> <b>{username}</b> - {rep} rep."
            )?,
            2 => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388956849963837711\">🌸</tg-emoji> <b>{username}</b> - {rep} rep."
            )?,
            _ => writeln!(&mut text, "&gt; <b>{username}</b> - {rep} rep.")?,
        }
    }

    bot.send_message(message.chat.id, text)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}
