use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::prelude::{Dialogue, Message};
use teloxide::types::{ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, User};
use anyhow::Result;
use log::info;
use crate::config::Config;
use crate::scheme::CANCEL_CALLBACK;
use crate::session::{Session, SessionState};

pub async fn on_support(
    bot: Bot,
    session: Session,
    message: Message
) -> Result<()> {
    if matches!(message.chat.kind, ChatKind::Private(..)) {
        session.update(SessionState::WaitSupportMessage).await?;

        bot.send_message(message.chat.id, "Отправьте сообщение для тех. поддержки.")
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::new(
                    "Отмена",
                    InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string())
                )
            ]]))
            .await?;
    } else {
        bot.send_message(message.chat.id, "Команда может быть использована только в личных сообщениях.").await?;
    }

    Ok(())
}

pub async fn on_support_message(
    bot: Bot,
    session: Session,
    message: Message,
    config: Arc<Config>,
) -> Result<()> {
    session.exit().await?;

    info!("Received support message from: {} text: {}",
        message.from.as_ref()
            .map(|u| u.username.as_ref())
            .flatten()
            .map(String::as_str)
            .unwrap_or("unknown"),
        message.text().unwrap_or("none")
    );

    bot.forward_message(config.admin_chat, message.chat.id, message.id).await?;
    bot.send_message(message.chat.id, "Сообщение отправлено!").await?;

    Ok(())
}

pub async fn on_support_cancel(
    bot: Bot,
    query: CallbackQuery,
    session: Session
) -> Result<()> {
    session.exit().await?;
    bot.send_message(session.chat_id(), "Отменено.").await?;
    bot.answer_callback_query(query.id).await?;
    Ok(())
}
