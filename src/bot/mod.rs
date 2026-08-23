use crate::{
    bot::scheme::{HELP_CALLBACK, ME_CALLBACK, SUPPORT_CALLBACK, TOP_CALLBACK, UNIT_INFO_CALLBACK},
    config::Config,
    oknoid::OknoId,
};
use anyhow::{anyhow, bail};
use std::sync::Arc;
use teloxide::{
    RequestError,
    dispatching::dialogue::GetChatId,
    prelude::*,
    types::{
        BotCommand, Chat, ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind,
        InlineKeyboardMarkup, InputFile, ParseMode,
    },
};

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
        /main_menu - главное меню.\n\
        /support - отправть сообщение в тех. поддержку, работает только в ЛС.\n\
        /me - показать свой профиль.\n\
        /info <code>&lt;пользователь&gt;</code> - запросить профиль пользователя.\n\
        /bio - установить описание профиля.\n\
        /top - топ пользователей по репутации.\n\
        /unit - информация о OKNO Unit.\n\
        /feedback <code>&lt;drop id&gt;</code> - оставить фидбек по id дропа.\n\
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
        BotCommand::new("menu", "главное меню"),
        BotCommand::new("support", "отправить сообщение в тех поддержку"),
        BotCommand::new("me", "показать свой профиль"),
        BotCommand::new("bio", "установить описание профиля"),
        BotCommand::new("top", "топ пользователей по репутации"),
        BotCommand::new("unit", "информация о OknoUnit"),
    ])
    .await?;

    Ok(())
}

pub async fn main_menu(bot: &Bot, config: &Config, chat: &Chat) -> anyhow::Result<()> {
    if !matches!(chat.kind, ChatKind::Private(..)) {
        bot.send_message(
            chat.id,
            "Команда может быть использована только в личных сообщениях.",
        )
        .await?;
        return Ok(());
    }

    let text = "\
        <b>OknoServant</b> - это ваш <b>помощник</b> в нашем <b>комьюнити</b>. Система <b>репутации</b>, <b>OknoUnit</b> и <b>тех.поддержка</b> - все в одном месте.\n\
        \n\
        <i>С чем я могу вам помочь?</i>";

    bot.send_photo(chat.id, InputFile::file_id(config.banners.main.clone()))
        .caption(text)
        .reply_markup(InlineKeyboardMarkup::new([
            vec![InlineKeyboardButton::callback(
                "Топ OknoMembers",
                TOP_CALLBACK,
            )],
            vec![
                InlineKeyboardButton::callback("OknoUnit", UNIT_INFO_CALLBACK),
                InlineKeyboardButton::callback("Тех. поддержка", SUPPORT_CALLBACK),
            ],
            vec![
                InlineKeyboardButton::url("t.me/oknogmdv", "https://t.me/oknogmdv".parse()?),
                InlineKeyboardButton::url("oknoweb.ru", "https://oknoweb.ru".parse()?),
            ],
            vec![InlineKeyboardButton::callback("Мой профиль", ME_CALLBACK)],
        ]))
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}
pub async fn main_menu_command(
    bot: Bot,
    config: Arc<Config>,
    message: Message,
) -> anyhow::Result<()> {
    main_menu(&bot, &config, &message.chat).await
}

pub async fn main_menu_callback(
    bot: Bot,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat = callback
        .message
        .as_ref()
        .map(|m| m.chat())
        .ok_or_else(|| anyhow!("failed to get chat id from callback"))?;

    main_menu(&bot, &config, chat).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}
