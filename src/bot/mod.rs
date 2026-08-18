use crate::{
    bot::scheme::HELP_CALLBACK,
    oknoid::{OknoId, UserRole},
};
use anyhow::bail;
use std::sync::Arc;
use teloxide::{
    RequestError,
    dispatching::dialogue::GetChatId,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, ParseMode},
};

mod args;
mod command;
mod profile;
pub mod scheme;
pub mod session;
pub mod support;
pub mod utils;

pub async fn invalid_usage_message(bot: &Bot, chat_id: ChatId) -> Result<(), RequestError> {
    bot.send_message(
        chat_id,
        "Неправильное использование комманды. Ознакомтесь со справкой.",
    )
    .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
        "Справка",
        InlineKeyboardButtonKind::CallbackData(HELP_CALLBACK.to_string()),
    )]]))
    .await?;
    Ok(())
}

pub async fn send_help_message(
    bot: &Bot,
    chat_id: ChatId,
    role: UserRole,
) -> Result<(), RequestError> {
    let mut text = "\
        <b>Общая информация:</b>\n\
        Для регистрации нужно прописать /start в личных сообщениях.\n\
        Пользователь указывается как @username или id пользователя.\n\
        \n\
        <b>Команды:</b>\n\
        /help - эта справка.\n\
        /support - переслать следующее сообщение в поддержку, работает только в ЛС.\n\
        /me - запросить свой профиль.\n\
        /info <code>&lt;пользователь&gt;</code> - запросить профиль пользователя.\n\
        /bio - установить следующее сообщение как описание профиля.\n\
    "
    .to_owned();

    if role >= UserRole::Admin {
        text.push_str("\n\
            <b>Команды админов:</b>\n\
            /rep <code>&lt;пользователь&gt;</code> <code>&lt;значение&gt;</code> - изменить репутацию пользователя на указанное значение.\n\
        ");
    }

    if role == UserRole::SuperAdmin {
        text.push_str(
            "\n\
            <b>Команды СУПЕРадминов:</b>\n\
            /admin_add <code>&lt;пользователь&gt;</code> - добавить админа.\n\
            /admin_del <code>&lt;пользователь&gt;</code> - убрать админа.\n\
        ",
        );
    }

    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(())
}

pub async fn on_help_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    let Some(chat_id) = callback.chat_id() else {
        return Ok(());
    };
    send_help_message(&bot, chat_id, db.get_user_role(callback.from.id).await?).await?;

    Ok(())
}

pub async fn on_help_command(bot: Bot, db: Arc<OknoId>, message: Message) -> anyhow::Result<()> {
    let Some(user) = message.from else {
        bail!("failed tp get user")
    };

    send_help_message(&bot, message.chat.id, db.get_user_role(user.id).await?).await?;

    Ok(())
}
