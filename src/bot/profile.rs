use crate::{
    bot::{
        args::{Mention, get_args},
        invalid_usage_message,
        scheme::CANCEL_CALLBACK,
        session::{Session, SessionState},
    },
    oknoid::{OknoId, UserInfo, UserRole},
    parser,
};
use anyhow::{anyhow, bail};
use log::error;
use std::sync::Arc;
use teloxide::{
    Bot,
    payloads::SendMessageSetters,
    prelude::{CallbackQuery, ChatId, Message, Requester, UserId},
    types::{ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, User},
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
    {
        error!("Error registering user: {:?}", error);
    };

    bot.send_message(
        message.chat.id,
        "Используйте /bio для изменения описаня профиля.",
    )
    .await?;
    Ok(())
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
    let text = if let Some(bio) = info.bio.as_ref() {
        format!(
            "Пользователь: @{username}\n\
            Роль: {}\n\
            Репутация: {}.\n\
            Описание:\n\
            {bio}",
            info.role, info.reputation
        )
    } else {
        format!(
            "Пользователь: {username}\n\
            Роль: {}\n\
            Репутация: {}.\n\
            Нет описания.",
            info.role, info.reputation
        )
    };
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

async fn change_role_with_condition(
    bot: &Bot,
    db: &OknoId,
    message: &Message,
    mention: Mention,
    role: UserRole,
    condition: impl FnOnce(UserRole) -> bool,
) -> anyhow::Result<bool> {
    let Some(User { id: user_id, .. }) = message.from else {
        bail!("Failed get user id");
    };
    if db.get_user_role(user_id).await? < UserRole::Admin {
        bot.send_message(message.chat.id, "У вас недостаточно прав")
            .await?;
        return Ok(false);
    }

    let Some(target_id) = mention.resolve(db) else {
        bot.send_message(message.chat.id, "Неизвестный пользователь")
            .await?;
        return Ok(false);
    };

    let current_role = db.get_user_role(target_id).await?;
    let proceed = condition(current_role);

    if proceed {
        db.set_user_role(target_id, role).await?;
    } else {
        bot.send_message(
            message.chat.id,
            format!("Пользователь имеет роль: {current_role}"),
        )
        .await?;
    }

    Ok(proceed)
}

pub async fn add_admin(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        if change_role_with_condition(&bot, &db, &message, mention, UserRole::Admin, |r| {
            r < UserRole::Admin
        })
        .await?
        {
            bot.send_message(message.chat.id, "Пользователь назначен админом.")
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
        if change_role_with_condition(&bot, &db, &message, mention, UserRole::Standard, |r| {
            r == UserRole::Admin
        })
        .await?
        {
            bot.send_message(message.chat.id, "Пользователь назначен админом.")
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

    if db.get_user_role(user_id).await? < UserRole::Admin {
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
