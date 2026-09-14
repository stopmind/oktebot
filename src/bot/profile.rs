use crate::{
    bot::{
        args::{Mention, get_args},
        invalid_usage_message,
        scheme::{
            BIO_CALLBACK, CANCEL_CALLBACK, MENU_CALLBACK, PROFILE_CALLBACK_PREFIX,
            TOP_CALLBACK_PREFIX,
        },
        session::{Session, SessionState},
        utils::{
            UtilError, UtilResult, check_private, check_user_privileges, check_user_super_admin,
            get_callback_chat, get_exactly_one_user, get_id_names, menu_button, try_delete_origin,
        },
    },
    config::Config,
    oknoid::{OknoId, Role, UserInfo},
    parser,
};
use log::error;
use std::{fmt::Write, iter, sync::Arc};
use teloxide::{
    Bot,
    dispatching::dialogue::GetChatId,
    payloads::{SendMessageSetters, SendPhotoSetters},
    prelude::{CallbackQuery, ChatId, Message, Requester, UserId},
    types::{
        Chat, ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
        InputFile, ParseMode, User,
    },
};

async fn check_registration_inner(db: &OknoId, user: Option<&User>) -> UtilResult<()> {
    let (id, names) = get_id_names(user)?;

    if db.check_user_exists(id) {
        db.update_names(id, names.username.as_deref(), names.first_name.as_ref())
            .await?;
    } else {
        db.register_user(
            id,
            UserInfo {
                username: names.username.map(|r| r.into_owned()),
                first_name: names.first_name.into_owned(),
                roles: Default::default(),
                is_banned: false,
                reputation: 0,
                bio: None,
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn check_registration(db: Arc<OknoId>, message: Message) {
    if !matches!(message.chat.kind, ChatKind::Private(..)) {
        return;
    }
    if let Err(err) = check_registration_inner(db.as_ref(), message.from.as_ref()).await {
        error!("Error checking registration: {}", err);
    }
}

async fn bio(bot: &Bot, session: &Session, chat: &Chat) -> anyhow::Result<()> {
    check_private(bot, chat).await?;
    session.update(SessionState::WaitBioMessage).await?;

    bot.send_message(
        chat.id,
        "Отправьте описание для профиля следующим сообщением.",
    )
    .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
        "Отмена",
        InlineKeyboardButtonKind::CallbackData(CANCEL_CALLBACK.to_string()),
    )]]))
    .await?;
    Ok(())
}

pub async fn on_bio_command(bot: Bot, session: Session, message: Message) -> anyhow::Result<()> {
    bio(&bot, &session, &message.chat).await?;
    Ok(())
}

pub async fn on_bio_callback(
    bot: Bot,
    session: Session,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat = get_callback_chat(&callback)?;
    bio(&bot, &session, chat).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_bio_message(
    bot: Bot,
    session: Session,
    message: Message,
    db: Arc<OknoId>,
) -> anyhow::Result<()> {
    let Some(text) = message.text() else {
        bot.send_message(message.chat.id, "Отправьте сообщение с текстом!")
            .await?;
        return Ok(());
    };

    if text.trim().is_empty() {
        bot.send_message(message.chat.id, "Описание не может быть пустым! >:(")
            .await?;
        return Ok(());
    }

    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    session.exit().await?;

    let result = db.set_bio(user.id, Some(text)).await;
    if let Err(error) = result {
        bot.send_message(message.chat.id, "Не удалось изменить описание.")
            .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
            .await?;
        Err(error.into())
    } else {
        bot.send_message(message.chat.id, "Описание профиля обновлено.")
            .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
            .await?;
        Ok(())
    }
}

async fn send_profile(
    bot: &Bot,
    db: &OknoId,
    chat_id: ChatId,
    user_id: UserId,
    is_me: bool,
    menu_button: bool,
) -> anyhow::Result<()> {
    let info = db.get_user_info(user_id).await?;

    let mut text = if let Some(username) = info.username.as_deref() {
        format!("> Username: {username}\n")
    } else {
        format!("> First name: {}\n", info.first_name)
    };

    if let Some(bio) = info.bio.as_ref() {
        writeln!(text, "> Bio: {bio}")?;
    }

    writeln!(text, "> Reputation: {} ⚡", info.reputation)?;

    for role in info.roles {
        writeln!(text, "> {role}")?;
    }

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new([iter::chain(
            is_me.then(|| {
                InlineKeyboardButton::callback(
                    if info.bio.is_some() {
                        "Изменить bio"
                    } else {
                        "Добавить bio"
                    },
                    BIO_CALLBACK,
                )
            }),
            menu_button.then(|| InlineKeyboardButton::callback("В меню", MENU_CALLBACK)),
        )]))
        .await?;
    Ok(())
}

pub async fn on_info_command(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let user_id = get_exactly_one_user(&bot, &db, message.chat.id, &mention).await?;

        send_profile(&bot, &db, message.chat.id, user_id, false, false).await?;
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_me_command(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    send_profile(&bot, &db, message.chat.id, user.id, true, false).await
}

pub async fn on_me_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    send_profile(&bot, &db, chat_id, callback.from.id, true, true).await?;
    try_delete_origin(&bot, &callback).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_profile_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    let callback_data = callback
        .data
        .as_ref()
        .ok_or(UtilError::FailedParseCallbackData)?;

    let user_id = callback_data[PROFILE_CALLBACK_PREFIX.len()..]
        .parse()
        .map_err(|_| UtilError::FailedParseCallbackData)
        .map(UserId)?;

    send_profile(&bot, &db, chat_id, user_id, false, false).await?;
    bot.answer_callback_query(callback.id).await?;

    Ok(())
}

pub async fn on_add_admin_command(
    bot: Bot,
    message: Message,
    db: Arc<OknoId>,
) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let id = get_exactly_one_user(&bot, &db, message.chat.id, &mention).await?;

        if db.give_role(id, Role::Admin).await? {
            bot.send_message(message.chat.id, "Пользователь назначен админом.")
                .await?;
        } else {
            bot.send_message(message.chat.id, "Пользователь уже является админом.")
                .await?;
        }
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }
    Ok(())
}

pub async fn on_del_admin(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let args = get_args(&message);
    if let Some(mention) = parser![Mention](args) {
        let id = get_exactly_one_user(&bot, &db, message.chat.id, &mention).await?;

        if db.take_role(id, Role::Admin).await? {
            bot.send_message(message.chat.id, "Пользователь более не является админом.")
                .await?;
        } else {
            bot.send_message(message.chat.id, "Пользователь не админ.")
                .await?;
        }
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_change_rep(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    check_user_privileges(&bot, &db, user.id, message.chat.id).await?;

    let args = get_args(&message);
    if let Some((mention, value)) = parser![Mention, i64](args) {
        let target_id = get_exactly_one_user(&bot, &db, message.chat.id, &mention).await?;
        let new_rep = db.add_reputation(target_id, value).await?;

        bot.send_message(message.chat.id, format!("Обновленная репутация: {new_rep}"))
            .await?;
    } else {
        invalid_usage_message(&bot, message.chat.id).await?;
    }

    Ok(())
}

async fn top(
    bot: &Bot,
    db: &OknoId,
    config: &Config,
    chat_id: ChatId,
    page: u32,
) -> anyhow::Result<()> {
    const PAGE_SIZE: u32 = 20;
    let top_data = db.get_top(page * PAGE_SIZE, PAGE_SIZE).await?;
    let users_count = db.users_count().await?;
    let pages_count = users_count.div_ceil(PAGE_SIZE);

    let mut text = format!(
        "<b>Таблица репутации OknoMembers:</b> ({users_count} пользователей, страница {}/{pages_count})\n",
        page + 1
    );
    for (i, (_, rep, names)) in top_data.into_iter().enumerate() {
        match (i, page) {
            (0, 0) => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388614717164005740\">🪷</tg-emoji> <b>{names}</b> - {rep} rep."
            )?,
            (1, 0) => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388967879439852799\">🌸</tg-emoji> <b>{names}</b> - {rep} rep."
            )?,
            (2, 0) => writeln!(
                &mut text,
                "&gt; <tg-emoji emoji-id=\"5388956849963837711\">🌸</tg-emoji> <b>{names}</b> - {rep} rep."
            )?,
            _ => writeln!(&mut text, "&gt; <b>{names}</b> - {rep} rep.")?,
        }
    }

    let markup = InlineKeyboardMarkup::new(
        [
            (page > 0).then(|| {
                [InlineKeyboardButton::callback(
                    "< Предыдущая страница",
                    format!("{TOP_CALLBACK_PREFIX}{}", page - 1),
                )]
            }),
            (page + 1 < pages_count).then(|| {
                [InlineKeyboardButton::callback(
                    "Следующая страница >",
                    format!("{TOP_CALLBACK_PREFIX}{}", page + 1),
                )]
            }),
            Some([menu_button()]),
        ]
        .into_iter()
        .flatten(),
    );

    bot.send_photo(chat_id, InputFile::file_id(config.banners.top.clone()))
        .caption(text)
        .parse_mode(ParseMode::Html)
        .reply_markup(markup)
        .await?;

    Ok(())
}

pub async fn on_top_command(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    message: Message,
) -> anyhow::Result<()> {
    top(&bot, &db, &config, message.chat.id, 0).await
}

pub async fn on_top_callback(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;
    let data = callback.data.as_deref().ok_or(UtilError::NoCallbackData)?;

    let page = data[TOP_CALLBACK_PREFIX.len()..].parse()?;
    top(&bot, &db, &config, chat_id, page).await?;
    try_delete_origin(&bot, &callback).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

async fn change_banned_state_command(
    bot: &Bot,
    db: &OknoId,
    message: &Message,
    banned: bool,
) -> anyhow::Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    check_user_super_admin(bot, db, user.id, message.chat.id).await?;

    let args = get_args(message);

    if let Some(mention) = parser![Mention](args) {
        let target = get_exactly_one_user(bot, db, message.chat.id, &mention).await?;

        let changed = db.is_user_banned(target).await? != banned;
        if changed {
            db.set_user_banned(target, banned).await?;
        }

        let response = if banned {
            if changed {
                "Пользователь забанен."
            } else {
                "Пользователь уже был забанен."
            }
        } else {
            if changed {
                "Пользователь более не забанен."
            } else {
                "Пользователь не был забанен."
            }
        };

        bot.send_message(message.chat.id, response).await?;
    } else {
        invalid_usage_message(bot, message.chat.id).await?;
    }

    Ok(())
}

pub async fn on_ban_command(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    change_banned_state_command(&bot, &db, &message, true).await
}

pub async fn on_unban_command(bot: Bot, message: Message, db: Arc<OknoId>) -> anyhow::Result<()> {
    change_banned_state_command(&bot, &db, &message, false).await
}
