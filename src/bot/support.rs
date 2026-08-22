use crate::{
    bot::{
        scheme::{CANCEL_CALLBACK, PROFILE_CALLBACK_PREFIX, SUPPORT_SELECTED_CALLBACK_PREFIX},
        session::{Session, SessionState},
    },
    config::Config,
};
use anyhow::{Result, anyhow, bail};
use std::sync::Arc;
use teloxide::{
    prelude::{Message, *},
    types::{Chat, ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup},
};

async fn support(bot: &Bot, config: &Config, chat: &Chat) -> Result<()> {
    if matches!(chat.kind, ChatKind::Private(..)) {
        bot.send_message(chat.id, "Выберите категорию: ")
            .reply_markup(InlineKeyboardMarkup::new(
                config.support_categories_layout.iter().map(|row| {
                    row.iter().map(|i| {
                        InlineKeyboardButton::new(
                            config.support_categories[*i].as_ref().clone(),
                            InlineKeyboardButtonKind::CallbackData(format!(
                                "{SUPPORT_SELECTED_CALLBACK_PREFIX}{i}"
                            )),
                        )
                    })
                }),
            ))
            .await?;
    } else {
        bot.send_message(
            chat.id,
            "Команда может быть использована только в личных сообщениях.",
        )
        .await?;
    }

    Ok(())
}

pub async fn on_support(bot: Bot, config: Arc<Config>, message: Message) -> Result<()> {
    support(&bot, &config, &message.chat).await
}

pub async fn support_callback(
    bot: Bot,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> Result<()> {
    let chat = callback
        .message
        .as_ref()
        .map(|msg| msg.chat())
        .ok_or_else(|| anyhow!("callback chat not found"))?;

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
    let message = callback
        .regular_message()
        .ok_or_else(|| anyhow!("callback message not found"))?;

    let callback_data = callback
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("callback not found"))?;

    let idx: usize = callback_data[SUPPORT_SELECTED_CALLBACK_PREFIX.len()..]
        .parse()
        .map_err(|_| anyhow!("failed to parse callback data: {}", callback_data))?;

    let category = config
        .support_categories
        .get(idx)
        .ok_or_else(|| anyhow!("support category not found"))?
        .clone();

    session
        .update(SessionState::WaitSupportMessage { category })
        .await?;
    bot.send_message(message.chat.id, "Отправьте сообщение для тех. поддержки.")
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "Отмена",
            InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
        )]]))
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

    let Some(user) = message.from else {
        bail!("failed get user")
    };

    let callback = format!("{PROFILE_CALLBACK_PREFIX}{}", user.id);

    bot.forward_message(config.support_chat, message.chat.id, message.id)
        .await?;
    bot.send_message(config.support_chat, format!("Категория: {category}"))
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "Описание профиля",
            InlineKeyboardButtonKind::CallbackData(callback),
        )]]))
        .await?;
    bot.send_message(message.chat.id, "Сообщение отправлено!")
        .await?;

    Ok(())
}

pub async fn on_support_cancel(bot: Bot, query: CallbackQuery, session: Session) -> Result<()> {
    session.exit().await?;
    bot.send_message(session.chat_id(), "Отменено.").await?;
    bot.answer_callback_query(query.id).await?;
    Ok(())
}
