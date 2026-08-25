use crate::{
    bot::{
        scheme::{CANCEL_CALLBACK, PROFILE_CALLBACK_PREFIX, SUPPORT_SELECTED_CALLBACK_PREFIX},
        session::{Session, SessionState},
        utils::{UtilError, check_private, get_callback_chat, menu_button},
    },
    config::Config,
};
use anyhow::{Result, anyhow};
use std::{iter, sync::Arc};
use teloxide::{
    dispatching::dialogue::GetChatId,
    prelude::{Message, *},
    types::{Chat, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode},
};

async fn support(bot: &Bot, config: &Config, chat: &Chat) -> Result<()> {
    check_private(bot, chat).await?;

    let buttons = config
        .support_categories_layout
        .iter()
        .map(|row| {
            row.iter()
                .map(|i| {
                    InlineKeyboardButton::callback(
                        config.support_categories[*i].as_ref().clone(),
                        format!("{SUPPORT_SELECTED_CALLBACK_PREFIX}{i}"),
                    )
                })
                .collect()
        })
        .chain(iter::once(vec![menu_button()]));

    bot.send_photo(chat.id, InputFile::file_id(config.banners.support.clone()))
        .caption("\
        Здесь вы можете обратится напрямую к <b>администрации</b> бота и oknoweb.ru. <b>ВСЕ</b> обращения будут рассмотрены.\n\
        \n\
        <i>На какую тему ваше обращение?</i>")
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(buttons))
        .await?;

    Ok(())
}

pub async fn on_support_command(bot: Bot, config: Arc<Config>, message: Message) -> Result<()> {
    support(&bot, &config, &message.chat).await
}

pub async fn on_support_callback(
    bot: Bot,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> Result<()> {
    let chat = get_callback_chat(&callback)?;

    support(&bot, &config, chat).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_support_selected_callback(
    bot: Bot,
    callback: CallbackQuery,
    session: Session,
    config: Arc<Config>,
) -> Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    let callback_data = callback.data.as_ref().ok_or(UtilError::NoCallbackData)?;

    let idx: usize = callback_data[SUPPORT_SELECTED_CALLBACK_PREFIX.len()..]
        .parse()
        .map_err(|_| UtilError::FailedParseCallbackData)?;

    let category = config
        .support_categories
        .get(idx)
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
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_support_message(
    bot: Bot,
    session: Session,
    message: Message,
    config: Arc<Config>,
    category: Arc<String>,
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
