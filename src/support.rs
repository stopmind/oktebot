use std::ops::Deref;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::{Dialogue, Message};
use teloxide::types::{ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, User};
use anyhow::Result;
use log::info;
use crate::config::Config;

pub const SUPPORT_CANCEL: &str = "support-cancel";

pub type SupportDialogue = Dialogue<SupportState, InMemStorage<SupportState>>;

#[derive(Clone, Default)]
pub enum SupportState {
    #[default]
    None,
    WaitMessage
}

pub async fn on_start(bot: Bot, message: Message) -> Result<()> {
    bot.send_message(message.chat.id, "Hello!").await?;
    Ok(())
}

pub async fn on_support(
    bot: Bot,
    dialogue: SupportDialogue,
    message: Message
) -> Result<()> {
    if matches!(message.chat.kind, ChatKind::Private(..)) {
        dialogue.update(SupportState::WaitMessage).await?;

        bot.send_message(message.chat.id, "Отправьте сообщение для тех. поддержки.")
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::new(
                    "Отмена",
                    InlineKeyboardButtonKind::CallbackData(SUPPORT_CANCEL.to_string())
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
    dialogue: SupportDialogue,
    message: Message,
    config: Arc<Config>,
) -> Result<()> {
    dialogue.exit().await?;

    info!("Received support message from: {} text: {}",
        message.from.as_ref()
            .map(|u| u.username.as_ref())
            .flatten()
            .map(String::as_str)
            .unwrap_or("unknown"),
        message.text().unwrap_or("none")
    );

    let a =  bot.forward_message(config.admin_chat, message.chat.id, message.id);
    info!("{:?}", a.deref());
    a.await?;

    bot.send_message(message.chat.id, "Сообщение отправлено!").await?;

    Ok(())
}

pub async fn on_support_cancel(
    bot: Bot,
    query: CallbackQuery,
    dialogue: SupportDialogue
) -> Result<()> {
    dialogue.exit().await?;
    bot.send_message(dialogue.chat_id(), "Отменено.").await?;
    bot.answer_callback_query(query.id).await?;
    Ok(())
}
