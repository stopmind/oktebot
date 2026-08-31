use crate::{
    bot::{
        scheme::{CANCEL_CALLBACK, PROFILE_CALLBACK_PREFIX, SUPPORT_SELECTED_CALLBACK_PREFIX},
        session::{Session, SessionState},
        utils::{
            UtilError, check_banned, check_private, get_callback_chat, menu_button,
            try_delete_origin,
        },
    },
    config::Config,
    oknoid::OknoId,
};
use anyhow::{Result, anyhow};
use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};
use teloxide::{
    dispatching::dialogue::GetChatId,
    prelude::{Message, *},
    types::{Chat, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode},
};

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

pub async fn on_support_command(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    message: Message,
) -> Result<()> {
    support(
        &bot,
        &db,
        &config,
        &message.chat,
        message.from.ok_or(UtilError::FailedGetUser)?.id,
    )
    .await
}

pub async fn on_support_callback(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> Result<()> {
    bot.answer_callback_query(callback.id.clone()).await?;
    let chat = get_callback_chat(&callback)?;

    check_banned(&bot, &db, chat.id, callback.from.id).await?;
    support(&bot, &db, &config, chat, callback.from.id).await?;
    try_delete_origin(&bot, &callback).await?;
    Ok(())
}

pub async fn on_support_selected_callback(
    bot: Bot,
    callback: CallbackQuery,
    session: Session,
    db: Arc<OknoId>,
) -> Result<()> {
    bot.answer_callback_query(callback.id.clone()).await?;
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    check_banned(&bot, &db, chat_id, callback.from.id).await?;

    let callback_data = callback.data.as_ref().ok_or(UtilError::NoCallbackData)?;

    let idx: usize = callback_data[SUPPORT_SELECTED_CALLBACK_PREFIX.len()..]
        .parse()
        .map_err(|_| UtilError::FailedParseCallbackData)?;

    let category = SupportCategory::from_usize(idx)
        .ok_or_else(|| anyhow!("support category not found"))?
        .clone();

    session
        .update(SessionState::WaitSupportMessage { category })
        .await?;
    bot.send_message(chat_id, "Отправьте сообщение для тех. поддержки.")
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Отмена", CANCEL_CALLBACK),
        ]]))
        .await?;
    Ok(())
}

pub async fn on_support_message(
    bot: Bot,
    session: Session,
    message: Message,
    config: Arc<Config>,
    category: SupportCategory,
) -> Result<()> {
    session.exit().await?;
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    let callback = format!("{PROFILE_CALLBACK_PREFIX}{}", user.id);

    bot.forward_message(config.support_chat, message.chat.id, message.id)
        .await?;
    bot.send_message(config.support_chat, format!("Категория: {category}"))
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Описание профиля", callback),
        ]]))
        .await?;
    bot.send_message(message.chat.id, "Сообщение отправлено!")
        .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
        .await?;

    Ok(())
}
