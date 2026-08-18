use crate::{
    bot::{
        scheme::{CANCEL_CALLBACK, PROFILE_CALLBACK_PREFIX},
        session::{Session, SessionState},
    },
    config::Config,
};
use anyhow::{Result, bail};
use std::sync::Arc;
use teloxide::{
    prelude::{Message, *},
    types::{ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup},
};

pub async fn on_support(bot: Bot, session: Session, message: Message) -> Result<()> {
    if matches!(message.chat.kind, ChatKind::Private(..)) {
        session.update(SessionState::WaitSupportMessage).await?;

        bot.send_message(message.chat.id, "Отправьте сообщение для тех. поддержки.")
            .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
                "Отмена",
                InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
            )]]))
            .await?;
    } else {
        bot.send_message(
            message.chat.id,
            "Команда может быть использована только в личных сообщениях.",
        )
        .await?;
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

    let Some(user) = message.from else {
        bail!("failed get user")
    };

    let callback = format!("{PROFILE_CALLBACK_PREFIX}{}", user.id);

    bot.forward_message(config.support_chat, message.chat.id, message.id)
        .await?;
    bot.send_message(config.support_chat, "===============")
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
