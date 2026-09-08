use crate::{
    bot::{args::Mention, scheme::MENU_CALLBACK},
    oknoid::{IdError, OknoId},
};
use teloxide::{
    Bot,
    requests::Requester,
    types::{CallbackQuery, Chat, ChatId, ChatKind, InlineKeyboardButton, Recipient, User, UserId},
};
use thiserror::Error;

pub const USER_BANNED: &str = "Вы забанены";

pub fn menu_button() -> InlineKeyboardButton {
    InlineKeyboardButton::callback("В меню", MENU_CALLBACK)
}

/// Requires CallbackQuery
pub fn callback_filter(
    id: impl AsRef<str> + Send + Sync + 'static,
) -> impl Fn(CallbackQuery) -> bool + Send + Sync + 'static {
    move |query: CallbackQuery| matches!(query.data, Some(query_id) if query_id == id.as_ref())
}

/// Requires CallbackQuery
pub fn callback_prefix_filter(
    id: impl AsRef<str> + Send + Sync + 'static,
) -> impl Fn(CallbackQuery) -> bool + Send + Sync + 'static {
    move |query: CallbackQuery| matches!(query.data, Some(query_id) if query_id.starts_with(id.as_ref()))
}

#[derive(Error, Debug)]
pub enum UtilError {
    #[error("failed to get chat info")]
    FailedGetChat,
    #[error("failed to get user info")]
    FailedGetUser,
    #[error("telegram error: {0}")]
    TelegramError(#[from] teloxide::RequestError),
    #[error("no callback data")]
    NoCallbackData,
    #[error("failed to parse callback data")]
    FailedParseCallbackData,
    #[error("id error: {0}")]
    IdError(#[from] IdError),
    #[error("incorrect usage")]
    UsageError,
}

pub type UtilResult<T> = Result<T, UtilError>;

pub async fn check_private(bot: &Bot, chat: &Chat) -> UtilResult<()> {
    if matches!(&chat.kind, ChatKind::Private { .. }) {
        Ok(())
    } else {
        bot.send_message(
            chat.id,
            "Действие может быть выполнено только в личных сообщениях.",
        )
        .await?;
        Err(UtilError::UsageError)
    }
}

pub fn get_callback_chat(callback: &CallbackQuery) -> UtilResult<&Chat> {
    callback
        .message
        .as_ref()
        .map(|m| m.chat())
        .ok_or(UtilError::FailedGetChat)
}

const NO_RIGHTS_MSG: &str = "У вас недостаточно прав!";

pub async fn check_user_privileges(
    bot: &Bot,
    db: &OknoId,
    user_id: UserId,
    chat_id: ChatId,
) -> UtilResult<()> {
    if db.check_user_privileges(user_id).await? {
        Ok(())
    } else {
        bot.send_message(chat_id, NO_RIGHTS_MSG).await?;
        Err(UtilError::UsageError)
    }
}

pub async fn check_user_super_admin(
    bot: &Bot,
    db: &OknoId,
    user_id: UserId,
    chat_id: ChatId,
) -> UtilResult<()> {
    if db.is_super_admin(user_id) {
        Ok(())
    } else {
        bot.send_message(chat_id, NO_RIGHTS_MSG).await?;
        Err(UtilError::UsageError)
    }
}

pub fn get_id_username(user: Option<&User>) -> UtilResult<(UserId, &str)> {
    if let Some(User {
        id,
        username: Some(username),
        ..
    }) = user
    {
        Ok((*id, username.as_str()))
    } else {
        Err(UtilError::FailedGetUser)
    }
}

pub async fn resolve_mention(
    bot: &Bot,
    db: &OknoId,
    chat_id: ChatId,
    mention: &Mention,
) -> UtilResult<Vec<UserId>> {
    let results = match mention {
        Mention::Username(username) =>
            db.resolve_username(username).map(|id| vec![id]),
        Mention::UserId(id) =>
            db.check_user_exists(*id).then(|| vec![*id]),
        Mention::Firstname(first_name) => {
            let ids = db.get_ids_by_first_name(first_name);
            ids.is_empty().then(|| ids)
        },
    };

    if results.is_none() {
        bot.send_message(chat_id, "Пользователь не найден!").await?;
    }

    results.ok_or(UtilError::UsageError)
}

pub async fn try_delete_origin(bot: &Bot, callback: &CallbackQuery) -> UtilResult<()> {
    if let Some(message) = callback.regular_message() {
        bot.delete_message(message.chat.id, message.id).await?;
    }

    Ok(())
}

pub async fn check_banned(
    bot: &Bot,
    db: &OknoId,
    recipient: impl Into<Recipient>,
    user_id: UserId,
) -> UtilResult<()> {
    if db.is_user_banned(user_id).await? {
        bot.send_message(recipient.into(), USER_BANNED).await?;
        Err(UtilError::UsageError)
    } else {
        Ok(())
    }
}
