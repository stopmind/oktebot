use crate::{bot::scheme::HELP_CALLBACK, oknoid::OknoId};
use anyhow::bail;
use std::sync::Arc;
use teloxide::{
    RequestError,
    dispatching::dialogue::GetChatId,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, ParseMode},
};
use teloxide::types::BotCommand;

mod args;
mod command;
mod oknounit;
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
    db: &OknoId,
    user_id: UserId,
) -> anyhow::Result<()> {
    let is_superadmin = db.is_super_admin(user_id);
    let is_admin = db.check_user_privileges(user_id).await?;

    let mut text = "\
        <b>Общая информация:</b>\n\
        Для регистрации нужно прописать /start в личных сообщениях.\n\
        Пользователь указывается как @username или id пользователя.\n\
        \n\
        <b>Команды:</b>\n\
        /help - эта справка.\n\
        /support - отправть сообщение в тех. поддержку, работает только в ЛС.\n\
        /me - показать свой профиль.\n\
        /info <code>&lt;пользователь&gt;</code> - запросить профиль пользователя.\n\
        /bio - установить описание профиля.\n\
        /top - топ пользователей по репутации.\n\
        /unit - информация о OKNO Unit.\n\
    "
    .to_owned();

    if is_admin {
        text.push_str("\n\
            <b>Команды админов:</b>\n\
            /rep <code>&lt;пользователь&gt;</code> <code>&lt;значение&gt;</code> - изменить репутацию пользователя на указанное значение.\n\
        ");
    }

    if is_superadmin {
        text.push_str(
            "\n\
            <b>Команды СУПЕРадминов:</b>\n\
            /admin_add <code>&lt;пользователь&gt;</code> - добавить админа.\n\
            /admin_del <code>&lt;пользователь&gt;</code> - убрать админа.\n\
            /drop <code>&lt;ссылка&gt;</code> - создать новый дроп.\n\
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
    send_help_message(&bot, chat_id, &db, callback.from.id).await?;

    Ok(())
}

pub async fn on_help_command(bot: Bot, db: Arc<OknoId>, message: Message) -> anyhow::Result<()> {
    let Some(user) = message.from else {
        bail!("failed tp get user")
    };

    send_help_message(&bot, message.chat.id, &db, user.id).await?;

    Ok(())
}

pub async fn set_commands(bot: &Bot) -> anyhow::Result<()> {
    bot.set_my_commands([
        BotCommand::new("help", "полная справка"),
        BotCommand::new("support", "отправить сообщение в тех поддержку, работает только в ЛС"),
        BotCommand::new("me", "показать свой профиль"),
        BotCommand::new("bio", "установить описание профиля"),
        BotCommand::new("top", "топ пользователей по репутации"),
        BotCommand::new("unit", "информация о OKNO Unit"),
    ])
        .await?;

    Ok(())
}