use crate::{
    bot::{
        router::{CallbackInfo, CommandInfo},
        scheme::{HELP_CALLBACK, ME_CALLBACK, SUPPORT_CALLBACK, TOP_CALLBACK, UNIT_INFO_CALLBACK},
        session::{SessionState, Sessions},
        utils::{check_private, menu_button},
    },
    config::Config,
    oknoid::OknoId,
};
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{
        BotCommand, Chat, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, Me, ParseMode,
    },
    RequestError,
};

mod args;
mod command;
mod oknounit;
mod profile;
mod router;
pub mod scheme;
pub mod session;
pub mod support;
pub mod utils;

pub struct BotContext {
    pub bot: Bot,
    pub config: Arc<Config>,
    pub db: OknoId,
    pub me: Me,
    pub sessions: Sessions,
}

pub async fn invalid_usage_message(bot: &Bot, chat_id: ChatId) -> Result<(), RequestError> {
    bot.send_message(
        chat_id,
        "Неправильное использование команды. Ознакомьтесь со справкой.",
    )
    .reply_markup(InlineKeyboardMarkup::new([[
        InlineKeyboardButton::callback("Справка", HELP_CALLBACK),
    ]]))
    .await?;
    Ok(())
}

pub async fn on_cancel_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    _args: (),
) -> anyhow::Result<()> {
    ctx.sessions.set(info.from.id, SessionState::Empty);
    ctx.bot
        .send_message(info.message.chat().id, "Отменено.")
        .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
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
        /menu - главное меню.\n\
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
            /drop <code>&lt;ссылка&gt;</code> <code>&lt;описание(опционально)&gt;</code> - создать новый дроп.\n\
            /ban <code>&lt;пользователь&gt;</code> - забанить пользователя.\n\
            /unban <code>&lt;пользователь&gt;</code> - разбанить пользователя.\n\
        ",
        );
    }

    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(())
}

pub async fn on_help_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    _args: (),
) -> anyhow::Result<()> {
    let chat_id = info.message.chat().id;
    send_help_message(&ctx.bot, chat_id, &ctx.db, info.from.id).await?;
    Ok(())
}

pub async fn on_help_command(ctx: &BotContext, info: &CommandInfo, _: ()) -> anyhow::Result<()> {
    send_help_message(&ctx.bot, info.chat.id, &ctx.db, info.from.id).await?;
    Ok(())
}

pub async fn set_commands(bot: &Bot) -> anyhow::Result<()> {
    bot.set_my_commands([
        BotCommand::new("menu", "главное меню"),
        BotCommand::new("help", "полная справка"),
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
    check_private(bot, chat).await?;

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

pub async fn on_main_menu_command(
    ctx: &BotContext,
    info: &CommandInfo,
    _: (),
) -> anyhow::Result<()> {
    main_menu(&ctx.bot, &ctx.config, &info.chat).await
}

pub async fn on_main_menu_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    _: (),
) -> anyhow::Result<()> {
    main_menu(&ctx.bot, &ctx.config, info.message.chat()).await?;
    Ok(())
}
