use crate::{bot::{
    args::Mention,
    router::{CallbackInfo, CommandInfo},
    scheme::{BIO_CALLBACK, CANCEL_CALLBACK, MENU_CALLBACK, TOP_CALLBACK_PREFIX},
    session::{SessionState, Sessions},
    utils::{
        check_private, get_exactly_one_user, get_id_names, menu_button, UtilError, UtilResult,
    },
    BotContext,
}, config::Config, fmt_callback, oknoid::{OknoId, Role, UserInfo}};
use log::error;
use std::{fmt::Write, iter, sync::Arc};
use teloxide::{
    payloads::{SendMessageSetters, SendPhotoSetters},
    prelude::{ChatId, Message, Requester, UserId},
    types::{
        Chat, ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
        InputFile, ParseMode, User,
    },
    Bot,
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

async fn bio(bot: &Bot, sessions: &Sessions, user_id: UserId, chat: &Chat) -> anyhow::Result<()> {
    check_private(bot, chat).await?;
    sessions.set(user_id, SessionState::WaitBioMessage);

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

pub async fn on_bio_command(ctx: &BotContext, info: &CommandInfo, _args: ()) -> anyhow::Result<()> {
    bio(&ctx.bot, &ctx.sessions, info.from.id, &info.chat).await?;
    Ok(())
}

pub async fn on_bio_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    _: (),
) -> anyhow::Result<()> {
    bio(&ctx.bot, &ctx.sessions, info.from.id, info.message.chat()).await?;
    Ok(())
}

pub async fn on_bio_message(
    ctx: &BotContext,
    message: &Message,
    (user,): (&User,)
) -> anyhow::Result<()> {
    let Some(text) = message.text() else {
        ctx.bot.send_message(message.chat.id, "Отправьте сообщение с текстом!")
            .await?;
        return Ok(());
    };

    if text.trim().is_empty() {
        ctx.bot.send_message(message.chat.id, "Описание не может быть пустым! >:(")
            .await?;
        return Ok(());
    }

    ctx.sessions.set(user.id, SessionState::Empty);

    let result = ctx.db.set_bio(user.id, Some(text)).await;
    if let Err(error) = result {
        ctx.bot.send_message(message.chat.id, "Не удалось изменить описание.")
            .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
            .await?;
        Err(error.into())
    } else {
        ctx.bot.send_message(message.chat.id, "Описание профиля обновлено.")
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

pub async fn on_info_command(
    ctx: &BotContext,
    info: &CommandInfo,
    (mention,): (Mention,),
) -> anyhow::Result<()> {
    let user_id = get_exactly_one_user(&ctx.bot, &ctx.db, info.chat.id, &mention).await?;
    send_profile(&ctx.bot, &ctx.db, info.chat.id, user_id, false, false).await?;
    Ok(())
}

pub async fn on_me_command(ctx: &BotContext, info: &CommandInfo, _args: ()) -> anyhow::Result<()> {
    send_profile(&ctx.bot, &ctx.db, info.chat.id, info.from.id, true, false).await
}

pub async fn on_me_callback(ctx: &BotContext, info: &CallbackInfo, _: ()) -> anyhow::Result<()> {
    let chat_id = info.message.chat().id;
    send_profile(&ctx.bot, &ctx.db, chat_id, info.from.id, true, true).await?;
    Ok(())
}

pub async fn on_profile_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    (user_id,): (u64,),
) -> anyhow::Result<()> {
    let chat_id = info.message.chat().id;
    let user_id = UserId(user_id);

    send_profile(&ctx.bot, &ctx.db, chat_id, user_id, false, false).await?;
    Ok(())
}

pub async fn on_add_admin_command(
    ctx: &BotContext,
    info: &CommandInfo,
    (mention,): (Mention,),
) -> anyhow::Result<()> {
    let id = get_exactly_one_user(&ctx.bot, &ctx.db, info.chat.id, &mention).await?;

    if ctx.db.give_role(id, Role::Admin).await? {
        ctx.bot
            .send_message(info.chat.id, "Пользователь назначен админом.")
            .await?;
    } else {
        ctx.bot
            .send_message(info.chat.id, "Пользователь уже является админом.")
            .await?;
    }

    Ok(())
}

pub async fn on_del_admin(
    ctx: &BotContext,
    info: &CommandInfo,
    (mention,): (Mention,),
) -> anyhow::Result<()> {
    let id = get_exactly_one_user(&ctx.bot, &ctx.db, info.chat.id, &mention).await?;

    if ctx.db.take_role(id, Role::Admin).await? {
        ctx.bot
            .send_message(info.chat.id, "Пользователь более не является админом.")
            .await?;
    } else {
        ctx.bot
            .send_message(info.chat.id, "Пользователь не админ.")
            .await?;
    }

    Ok(())
}

pub async fn on_change_rep(
    ctx: &BotContext,
    info: &CommandInfo,
    (mention, rep): (Mention, i64),
) -> anyhow::Result<()> {
    let target_id = get_exactly_one_user(&ctx.bot, &ctx.db, info.chat.id, &mention).await?;
    let new_rep = ctx.db.add_reputation(target_id, rep).await?;

    ctx.bot
        .send_message(info.chat.id, format!("Обновленная репутация: {new_rep}"))
        .await?;

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
                    fmt_callback![TOP_CALLBACK_PREFIX, page - 1]
                )]
            }),
            (page + 1 < pages_count).then(|| {
                [InlineKeyboardButton::callback(
                    "Следующая страница >",
                    fmt_callback![TOP_CALLBACK_PREFIX, page + 1]
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

pub async fn on_top_command(ctx: &BotContext, info: &CommandInfo, _args: ()) -> anyhow::Result<()> {
    top(&ctx.bot, &ctx.db, &ctx.config, info.chat.id, 0).await
}

pub async fn on_top_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    (page,): (u32,),
) -> anyhow::Result<()> {
    let chat = info.message.chat();
    top(&ctx.bot, &ctx.db, &ctx.config, chat.id, page).await?;
    Ok(())
}

async fn change_banned_state_command(
    ctx: &BotContext,
    info: &CommandInfo,
    mention: Mention,
    banned: bool,
) -> anyhow::Result<()> {
    let target = get_exactly_one_user(&ctx.bot, &ctx.db, info.chat.id, &mention).await?;

    let changed = ctx.db.is_user_banned(target).await? != banned;
    if changed {
        ctx.db.set_user_banned(target, banned).await?;
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

    ctx.bot.send_message(info.chat.id, response).await?;
    Ok(())
}

pub async fn on_ban_command(
    ctx: &BotContext,
    info: &CommandInfo,
    (mention,): (Mention,),
) -> anyhow::Result<()> {
    change_banned_state_command(ctx, info, mention, true).await
}

pub async fn on_unban_command(
    ctx: &BotContext,
    info: &CommandInfo,
    (mention,): (Mention,),
) -> anyhow::Result<()> {
    change_banned_state_command(ctx, info, mention, false).await
}
