use crate::{
    bot::{
        router::{CallbackInfo, CommandInfo},
        scheme::{CANCEL_CALLBACK, PROFILE_CALLBACK_PREFIX, SUPPORT_SELECTED_CALLBACK_PREFIX},
        session::SessionState,
        utils::{
            check_banned, check_private, menu_button
        },
        BotContext,
    },
    config::Config,
    oknoid::OknoId,
};
use anyhow::{anyhow, Result};
use std::{
    fmt::{Display, Formatter}
};
use teloxide::{
    prelude::{Message, *},
    types::{Chat, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode},
};
use teloxide::types::User;

#[derive(Clone, Copy)]
pub enum SupportCategory {
    ChangeSubmit = 0,
    SuggestDrop = 1,
    Bugreport = 2,
    Other = 3,
}

impl SupportCategory {
    const fn from_usize(val: usize) -> Option<Self> {
        macro_rules! chk {
            ($($i:ident),*) => {
                match val {
                    $(x if x == $i as usize => Some($i),)*
                    _ => None
                }
            };
        }

        use SupportCategory::*;
        chk!(ChangeSubmit, SuggestDrop, Bugreport, Other)
    }

    fn as_str(self) -> &'static str {
        match self {
            SupportCategory::ChangeSubmit => "Изменение сабмита",
            SupportCategory::SuggestDrop => "Предложить дроп",
            SupportCategory::Bugreport => "Багрепорт",
            SupportCategory::Other => "Другое",
        }
    }
}

impl Display for SupportCategory {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn choice_button(category: SupportCategory) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(
        category.as_str(),
        format!("{SUPPORT_SELECTED_CALLBACK_PREFIX}{}", category as usize),
    )
}

async fn support(
    bot: &Bot,
    db: &OknoId,
    config: &Config,
    chat: &Chat,
    user_id: UserId,
) -> Result<()> {
    check_banned(bot, db, chat.id, user_id).await?;
    check_private(bot, chat).await?;

    let buttons = vec![
        vec![choice_button(SupportCategory::ChangeSubmit)],
        vec![
            choice_button(SupportCategory::Bugreport),
            choice_button(SupportCategory::Other),
        ],
        vec![menu_button()],
    ];

    bot.send_photo(chat.id, InputFile::file_id(config.banners.support.clone()))
        .caption("\
        Здесь вы можете обратится напрямую к <b>администрации</b> бота и oknoweb.ru. <b>ВСЕ</b> обращения будут рассмотрены.\n\
        \n\
        <i>На какую тему ваше обращение?</i>")
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup { inline_keyboard: buttons })
        .await?;

    Ok(())
}

pub async fn on_support_command(ctx: &BotContext, info: &CommandInfo, _: ()) -> Result<()> {
    support(&ctx.bot, &ctx.db, &ctx.config, &info.chat, info.from.id).await
}

pub async fn on_support_callback(ctx: &BotContext, info: &CallbackInfo, _: ()) -> Result<()> {
    let chat = info.message.chat();
    support(&ctx.bot, &ctx.db, &ctx.config, chat, info.from.id).await?;
    Ok(())
}

pub async fn on_support_selected_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    (idx,): (usize,),
) -> Result<()> {
    let chat = info.message.chat();
    let category =
        SupportCategory::from_usize(idx).ok_or_else(|| anyhow!("support category not found"))?;

    ctx.sessions
        .set(info.from.id, SessionState::WaitSupportMessage { category });
    ctx.bot
        .send_message(chat.id, "Отправьте сообщение для тех. поддержки.")
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Отмена", CANCEL_CALLBACK),
        ]]))
        .await?;
    Ok(())
}

pub async fn on_support_message(
    ctx: &BotContext,
    message: &Message,
    (user, category): (&User, SupportCategory),
) -> Result<()> {
    ctx.sessions.set(user.id, SessionState::Empty);

    let callback = format!("{PROFILE_CALLBACK_PREFIX}{}", user.id);

    ctx.bot.forward_message(ctx.config.support_chat, message.chat.id, message.id)
        .await?;
    ctx.bot.send_message(ctx.config.support_chat, format!("Категория: {category}"))
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Описание профиля", callback),
        ]]))
        .await?;
    ctx.bot.send_message(message.chat.id, "Сообщение отправлено!")
        .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
        .await?;

    Ok(())
}
