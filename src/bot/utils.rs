use crate::{
    bot::{args::Mention, scheme::MENU_CALLBACK},
    oknoid::{IdError, OknoId},
};
use std::{fmt::Write, ops::Not};
use teloxide::{
    requests::Requester,
    types::{ChatId, InlineKeyboardButton, UserId},
    Bot,
};
use thiserror::Error;

pub fn menu_button() -> InlineKeyboardButton {
    InlineKeyboardButton::callback("В меню", MENU_CALLBACK)
}

#[derive(Error, Debug)]
pub enum UtilError {
    #[error("failed to get user info")]
    FailedGetUser,
    #[error("telegram error: {0}")]
    TelegramError(#[from] teloxide::RequestError),
    #[error("id error: {0}")]
    IdError(#[from] IdError),
    #[error("incorrect usage")]
    UsageError,
}

pub type UtilResult<T> = Result<T, UtilError>;

pub fn resolve_mention(db: &OknoId, mention: &Mention) -> UtilResult<Vec<UserId>> {
    let results = match mention {
        Mention::Username(username) => db.resolve_username(username).map(|id| vec![id]),
        Mention::UserId(id) => db.check_user_exists(*id).then(|| vec![*id]),
        Mention::Firstname(first_name) => {
            let ids = db.get_ids_by_first_name(first_name);
            ids.is_empty().not().then_some(ids)
        }
    };

    results.ok_or(UtilError::UsageError)
}

pub async fn user_not_found(bot: &Bot, chat_id: ChatId) -> UtilResult<()> {
    bot.send_message(chat_id, "Пользователь не найден!").await?;
    Ok(())
}

pub async fn multiple_users_found(
    bot: &Bot,
    db: &OknoId,
    chat_id: ChatId,
    users: impl Iterator<Item = UserId>,
) -> UtilResult<()> {
    let mut message = "Найдено несколько пользователей, выберите одного и используйте команду повторно, указав id или username:\n".to_owned();
    for user_id in users {
        let Some(names) = db.get_user_names(user_id) else {
            continue;
        };
        writeln!(message, "> {names} id: {user_id}").expect("writing to string cannot fail");
    }

    bot.send_message(chat_id, message).await?;
    Ok(())
}

pub async fn get_exactly_one_user(
    bot: &Bot,
    db: &OknoId,
    chat_id: ChatId,
    mention: &Mention,
) -> UtilResult<UserId> {
    let mut targets = resolve_mention(db, mention)?;

    if targets.len() > 1 {
        multiple_users_found(bot, db, chat_id, targets.into_iter()).await?;
        return Err(UtilError::FailedGetUser);
    }

    let Some(target) = targets.pop() else {
        user_not_found(bot, chat_id).await?;
        return Err(UtilError::FailedGetUser);
    };

    Ok(target)
}

#[macro_export]
macro_rules! fmt_callback {
    [$name:expr $(, $arg:expr)*] => {
        {
            use std::fmt::Write;
            let mut result = format!("{}", $name);
            $(write!(result, ":{}", $arg).unwrap();)*
            result
        }
    };
}