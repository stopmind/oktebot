use crate::{
    bot::{
        args::get_args,
        invalid_usage_message,
        scheme::{
            CANCEL_CALLBACK, DROPS_HISTORY_CALLBACK, PROFILE_CALLBACK_PREFIX,
            UNIT_ACCEPT_REPORT_CALLBACK_PREFIX, UNIT_JOIN_CALLBACK, UNIT_REPORT_CALLBACK_PREFIX,
        },
        session::{Session, SessionState},
    },
    config::Config,
    oknoid::{DropId, OknoId, Role},
};
use anyhow::{anyhow, bail};
use log::error;
use std::{fmt::Write, iter, sync::Arc};
use teloxide::{
    Bot,
    payloads::SendMessageSetters,
    requests::Requester,
    types::{
        CallbackQuery, Chat, ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind,
        InlineKeyboardMarkup, Message, User, UserId,
    },
};

pub async fn unit_info_command(bot: Bot, db: Arc<OknoId>, message: Message) -> anyhow::Result<()> {
    let Some(User { id: user_id, .. }) = message.from else {
        bail!("failed to get user id")
    };

    if db.check_role(user_id, Role::OknoUnit).await? {
        let drops = db.get_latest_drops(3).await?;

        let mut drop_completeness = Vec::with_capacity(drops.len());
        for (id, _) in &drops {
            drop_completeness.push(db.check_drop_completed(*id, user_id).await?);
        }

        let mut text = "Latest drops: \n".to_string();
        for (i, (_, link)) in drops.iter().enumerate() {
            if drop_completeness[i] {
                writeln!(&mut text, "{}. {} (completed)", i + 1, link)?;
            } else {
                writeln!(&mut text, "{}. {}", i + 1, link)?;
            }
        }

        let markup = InlineKeyboardMarkup::new(
            drops
                .iter()
                .enumerate()
                .filter(|(i, _)| !drop_completeness[*i])
                .map(|(i, (id, _))| {
                    [InlineKeyboardButton::new(
                        format!("Report drop {}", i + 1),
                        InlineKeyboardButtonKind::CallbackData(format!(
                            "{UNIT_REPORT_CALLBACK_PREFIX}{id}"
                        )),
                    )]
                })
                .chain(iter::once([InlineKeyboardButton::new(
                    "History",
                    InlineKeyboardButtonKind::CallbackData(DROPS_HISTORY_CALLBACK.to_string()),
                )])),
        );

        bot.send_message(message.chat.id, text)
            .reply_markup(markup)
            .await?;
    } else {
        bot.send_message(message.chat.id, "Присоединяйтесь к Okno UNIT")
            .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
                "Присоединится",
                InlineKeyboardButtonKind::CallbackData(UNIT_JOIN_CALLBACK.to_string()),
            )]]))
            .await?;
    }

    Ok(())
}

pub async fn unit_join_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let Message {
        chat: Chat { id: chat_id, .. },
        ..
    } = callback
        .regular_message()
        .ok_or_else(|| anyhow!("failed get callback chat id"))?;
    let chat_id = *chat_id;

    if db.give_role(callback.from.id, Role::OknoUnit).await? {
        bot.send_message(chat_id, "С вступлением в Onko Unit")
            .await?;
    }

    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn unit_accept_report_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let Message {
        chat: Chat { id: chat_id, .. },
        ..
    } = callback
        .regular_message()
        .ok_or_else(|| anyhow!("failed get callback chat id"))?;
    let chat_id = *chat_id;

    if !db.check_user_privileges(callback.from.id).await? {
        bot.send_message(chat_id, "У вас нет недостаточно прав!").await?;
        bot.answer_callback_query(callback.id).await?;
        return Ok(());
    }

    let callback_data = callback
        .data
        .ok_or_else(|| anyhow!("failed get callback data"))?;
    let (unit_id, drop_id) = callback_data[UNIT_ACCEPT_REPORT_CALLBACK_PREFIX.len()..]
        .split_once('-')
        .ok_or_else(|| anyhow!("failed parse callback data"))?;

    let (unit_id, drop_id) = (UserId(unit_id.parse()?), drop_id.parse()?);

    if db.mark_drop_completed(drop_id, unit_id).await? {
        bot.send_message(
            chat_id,
            "Дроп был отмечен как выполненный для данного пользователя.",
        )
        .await?;
        bot.send_message(unit_id, format!("Вы выполнили дроп {drop_id}"))
            .await?;
    } else {
        bot.send_message(chat_id, "Дроп уже был выполнен данным пользователем.")
            .await?;
    }

    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_unit_report_message(
    bot: Bot,
    db: Arc<OknoId>,
    session: Session,
    message: Message,
    config: Arc<Config>,
    drop_id: DropId,
) -> anyhow::Result<()> {
    session.exit().await?;

    let Some(user) = message.from else {
        bail!("failed get user")
    };

    let drop_link = db.get_drop(drop_id).await?;

    bot.forward_message(config.support_chat, message.chat.id, message.id)
        .await?;
    bot.send_message(config.support_chat, format!("Дроп: {drop_link}"))
        .reply_markup(InlineKeyboardMarkup::new([
            [InlineKeyboardButton::new(
                "Описание профиля",
                InlineKeyboardButtonKind::CallbackData(format!(
                    "{PROFILE_CALLBACK_PREFIX}{}",
                    user.id
                )),
            )],
            [InlineKeyboardButton::new(
                "Потвердить",
                InlineKeyboardButtonKind::CallbackData(format!(
                    "{UNIT_ACCEPT_REPORT_CALLBACK_PREFIX}{}-{}",
                    user.id, drop_id
                )),
            )],
        ]))
        .await?;
    bot.send_message(message.chat.id, "Сообщение отправлено!")
        .await?;

    Ok(())
}

pub async fn on_unit_report_cancel(
    bot: Bot,
    query: CallbackQuery,
    session: Session,
) -> anyhow::Result<()> {
    session.exit().await?;
    bot.send_message(session.chat_id(), "Отменено.").await?;
    bot.answer_callback_query(query.id).await?;
    Ok(())
}

async fn unit_report(
    bot: &Bot,
    db: &OknoId,
    session: &Session,
    drop_id: DropId,
    user_id: UserId,
) -> anyhow::Result<()> {
    if !db.check_drop_exists(drop_id).await? {
        bot.send_message(user_id, "Дропа с таким id не существует!")
            .await?;
        return Ok(());
    }

    if db.check_drop_completed(drop_id, user_id).await? {
        bot.send_message(user_id, "Вы уже выполнили этот дроп.")
            .await?;
        return Ok(());
    }

    session
        .update(SessionState::WaitUnitReport { drop_id })
        .await?;

    bot.send_message(user_id, "Отправьте сообщение для подтверждения выполнения.")
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "Отмена",
            InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
        )]]))
        .await?;

    Ok(())
}

pub async fn unit_report_callback(
    bot: Bot,
    db: Arc<OknoId>,
    session: Session,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let message = callback
        .regular_message()
        .ok_or_else(|| anyhow!("callback message not found"))?;

    if !matches!(message.chat.kind, ChatKind::Private(..)) {
        bot.send_message(
            message.chat.id,
            "Действие может быть выполнено только в личных сообщениях.",
        )
        .await?;

        bot.answer_callback_query(callback.id).await?;
        return Ok(());
    }

    let callback_data = callback
        .data
        .ok_or_else(|| anyhow!("failed to get callback data"))?;
    let drop_id = callback_data[UNIT_REPORT_CALLBACK_PREFIX.len()..].parse()?;

    unit_report(&bot, &db, &session, drop_id, callback.from.id).await?;

    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn unit_report_command(
    bot: Bot,
    db: Arc<OknoId>,
    session: Session,
    message: Message,
) -> anyhow::Result<()> {
    if !matches!(message.chat.kind, ChatKind::Private(..)) {
        bot.send_message(
            message.chat.id,
            "Команда может быть использована только в личных сообщениях.",
        )
        .await?;
        return Ok(());
    }

    let Ok(drop_id) = get_args(&message).parse() else {
        invalid_usage_message(&bot, message.chat.id).await?;
        return Ok(());
    };

    let User { id: user_id, .. } = message.from.ok_or_else(|| anyhow!("failed get user id"))?;

    unit_report(&bot, &db, &session, drop_id, user_id).await?;

    Ok(())
}

pub async fn drop_command(bot: Bot, db: Arc<OknoId>, message: Message) -> anyhow::Result<()> {
    let User { id: user_id, .. } = message
        .from
        .as_ref()
        .ok_or_else(|| anyhow!("failed get user id"))?;
    if !db.is_super_admin(*user_id) {
        bot.send_message(message.chat.id, "У вас недостаточно прав.")
            .await?;
        return Ok(());
    }

    let link = get_args(&message);
    if link.is_empty() {
        bot.send_message(message.chat.id, "Укажите ссылку").await?;
        return Ok(());
    }

    let drop_id = db.add_drop(link).await?;
    let text = format!("Новый дроп: {link}");
    let markup = InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
        "Report drop",
        InlineKeyboardButtonKind::CallbackData(format!("{UNIT_REPORT_CALLBACK_PREFIX}{drop_id}")),
    )]]);

    for unit in db.get_users_by_role(Role::OknoUnit).await? {
        let res = bot
            .send_message(unit, text.clone())
            .reply_markup(markup.clone())
            .await;

        if let Err(err) = res {
            error!("Failed to notify user {unit} about drop {drop_id} due: {err}");
        }
    }

    bot.send_message(
        message.chat.id,
        format!("Новый дроп создан с id: {drop_id}"),
    )
    .await?;

    Ok(())
}

pub async fn drops_history_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let Message {
        chat: Chat { id: chat_id, .. },
        ..
    } = callback
        .regular_message()
        .ok_or_else(|| anyhow!("failed to get callback chat id"))?;
    let chat_id = *chat_id;

    let drops = db.get_latest_drops(3).await?;

    let mut drop_completeness = Vec::with_capacity(drops.len());
    for (id, _) in &drops {
        drop_completeness.push(db.check_drop_completed(*id, callback.from.id).await?);
    }

    let mut text = "Drops history\nUse /unit_report <id> to report:\n".to_string();
    for (i, (id, link)) in drops.iter().enumerate() {
        if drop_completeness[i] {
            writeln!(&mut text, "{}. {} (completed)", id, link)?;
        } else {
            writeln!(&mut text, "{}. {}", id, link)?;
        }
    }

    bot.send_message(chat_id, text).await?;

    bot.answer_callback_query(callback.id).await?;
    Ok(())
}
