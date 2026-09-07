use crate::{
    bot::{
        args::get_args,
        invalid_usage_message,
        scheme::{
            CANCEL_CALLBACK, DROPS_HISTORY_CALLBACK, MENU_CALLBACK, PROFILE_CALLBACK_PREFIX,
            UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, UNIT_FEEDBACK_CALLBACK_PREFIX,
            UNIT_INFO_CALLBACK, UNIT_JOIN_CALLBACK,
        },
        session::{Session, SessionState},
        support,
        support::SupportCategory,
        utils::{
            USER_BANNED, UtilError, check_banned, check_private, check_user_privileges,
            check_user_super_admin, get_callback_chat, menu_button, try_delete_origin,
        },
    },
    config::Config,
    oknoid::{DropId, OknoId, Role},
};
use log::error;
use std::{fmt::Write, iter, sync::Arc};
use teloxide::{
    Bot,
    dispatching::dialogue::GetChatId,
    payloads::{EditMessageReplyMarkupSetters, SendMessageSetters, SendPhotoSetters},
    requests::{Request, Requester},
    types::{
        CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile,
        MaybeInaccessibleMessage, Message, ParseMode, UserId,
    },
};

async fn unit_info(
    bot: &Bot,
    db: &OknoId,
    config: &Config,
    user_id: UserId,
    chat_id: ChatId,
    from_menu: bool,
) -> anyhow::Result<()> {
    if db.check_role(user_id, Role::OknoUnit).await? {
        let drops = db.get_latest_drops(3).await?;

        let mut drop_completeness = Vec::with_capacity(drops.len());
        for drop in &drops {
            drop_completeness.push(db.check_drop_completed(drop.id, user_id).await?);
        }

        let mut text = String::new();
        for (i, drop) in drops.iter().enumerate() {
            writeln!(
                &mut text,
                "{}. {}{}",
                i + 1,
                drop.link,
                if drop_completeness[i] {
                    " (выполнен)"
                } else {
                    ""
                }
            )?;
            if let Some(description) = &drop.description {
                writeln!(&mut text, "> {}", description)?;
            }
        }

        let markup = InlineKeyboardMarkup::new(
            drops
                .iter()
                .enumerate()
                .filter(|(i, _)| !drop_completeness[*i])
                .map(|(i, drop)| {
                    [InlineKeyboardButton::callback(
                        format!("Я оставил фидбек для {}", i + 1),
                        format!("{UNIT_FEEDBACK_CALLBACK_PREFIX}{}", drop.id),
                    )]
                })
                .chain(iter::once([support::choice_button(
                    SupportCategory::SuggestDrop,
                )]))
                .chain(iter::once([InlineKeyboardButton::callback(
                    "История дропов",
                    DROPS_HISTORY_CALLBACK,
                )]))
                .chain(iter::once([menu_button()])),
        );

        bot.send_photo(chat_id, InputFile::file_id(config.banners.unit.clone()))
            .caption(text)
            .reply_markup(markup)
            .await?;
    } else {
        bot.send_photo(chat_id,  InputFile::file_id(config.banners.unit.clone()))
            .caption("\
                <b>OknoUnit</b> - Это статус <b>боевой единицы</b> нашего сообщества. Задача каждого OknoUnit`а - <b>проявлять активность на дропах.</b>\n\
                \n\
                > <b>Дроп</b> - это любое <b>видео</b>, <b>игра</b> или другая единица <b>контента</b> от нашего сообщества.\n\
                \n\
                > За <b>каждый комментарий/отзыв</b> ваша <b>репутация повышается</b>. <b>OknoUnit</b> - один из самых <b>эффективных способов</b> нафармить <b>репутацию.</b>\n\
                \n\
                > Каждый <b>OknoUnit</b> получает <b>уведомления о новых дропах</b> сообщества.\n\
                \n\
                <b>Cоветы:</b> \n\
                > <b>Не пропускайте дропы.</b> Каждый дроп - возможность получить до 3 очков репутации\n\
                \n\
                > <b>Пишите развернутые фидбеки.</b> Мы поощеряем длинный и развернутый фидбек, который вписывается в контекст дропа. Комментарии по типу \"аоаоа прикольно!\" мы не учитываем вовсе.\n\
                \n\
                > <b>Предлагайте свои дропы.</b> Делайте игры, ролики и скидываете их нам в бота. За ролик про джем или OknoSubmit мы щедро отсыпаем 5 и больше баллов\n\
                ")
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
                "Стать OknoUnit",
                UNIT_JOIN_CALLBACK,
            )]].into_iter().chain(from_menu.then(|| [menu_button()]))))
            .await?;
    }

    Ok(())
}

pub async fn on_unit_info_command(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    message: Message,
) -> anyhow::Result<()> {
    let user = message.from.ok_or(UtilError::FailedGetUser)?;

    unit_info(&bot, &db, &config, user.id, message.chat.id, false).await
}

pub async fn on_unit_info_callback(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    unit_info(&bot, &db, &config, callback.from.id, chat_id, true).await?;
    try_delete_origin(&bot, &callback).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_unit_join_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;
    if db.give_role(callback.from.id, Role::OknoUnit).await? {
        bot.send_message(chat_id, "Вы стали OknoUnit!")
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::callback("В меню", MENU_CALLBACK),
            ]]))
            .await?;
    }

    bot.answer_callback_query(callback.id).await?;
    Ok(())
}

pub async fn on_unit_accept_report_callback(
    bot: Bot,
    db: Arc<OknoId>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    bot.answer_callback_query(callback.id.clone()).await?;
    let message = callback
        .message
        .as_ref()
        .and_then(MaybeInaccessibleMessage::regular_message)
        .ok_or(UtilError::FailedGetChat)?;

    check_user_privileges(&bot, &db, callback.from.id, message.chat.id).await?;

    let callback_data = callback.data.ok_or(UtilError::NoCallbackData)?;
    let [unit_id, drop_id, rep_count] = callback_data[UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX.len()..]
        .split('-')
        .collect::<Vec<&str>>()
        .try_into()
        .map_err(|_| UtilError::FailedParseCallbackData)?;

    let unit_id = UserId(unit_id.parse()?);
    let drop_id = drop_id.parse()?;
    let rep_count = rep_count.parse()?;

    let username = db.get_username(unit_id).ok_or(UtilError::FailedGetUser)?;

    if db.mark_drop_completed(drop_id, unit_id).await? {
        let new_rep = db.add_reputation(unit_id, rep_count).await?;
        bot.send_message(
            message.chat.id,
            format!("Дроп был отмечен как выполненный для @{username}.\nОбновленная репутация: {new_rep}"),
        )
        .await?;
        bot.send_message(
            unit_id,
            format!("Ваша заявка по дропу {drop_id} была принята."),
        )
        .await?;

        bot.edit_message_reply_markup(message.chat.id, message.id)
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::callback(
                    "Описание профиля",
                    format!("{PROFILE_CALLBACK_PREFIX}{}", unit_id),
                ),
            ]]))
            .await?;
    } else {
        bot.send_message(
            message.chat.id,
            "Дроп уже был выполнен данным пользователем.",
        )
        .await?;
    }
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

    let user = message.from.ok_or(UtilError::FailedGetUser)?;

    let drop = db.get_drop(drop_id).await?;

    bot.forward_message(config.support_chat, message.chat.id, message.id)
        .await?;
    bot.send_message(
        config.support_chat,
        format!(
            "Дроп: {}{}{}",
            drop.link,
            if drop.description.is_some() {
                "\n> "
            } else {
                ""
            },
            drop.description.as_deref().unwrap_or("")
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new([
        [InlineKeyboardButton::callback(
            "Описание профиля",
            format!("{PROFILE_CALLBACK_PREFIX}{}", user.id),
        )],
        [InlineKeyboardButton::callback(
            "Подтвердить +1",
            format!(
                "{UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX}{}-{drop_id}-1",
                user.id
            ),
        )],
        [InlineKeyboardButton::callback(
            "Подтвердить +2",
            format!(
                "{UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX}{}-{drop_id}-2",
                user.id
            ),
        )],
        [InlineKeyboardButton::callback(
            "Подтвердить +3",
            format!(
                "{UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX}{}-{drop_id}-3",
                user.id
            ),
        )],
    ]))
    .await?;

    bot.send_message(
        message.chat.id,
        "Заявка отправлена! Админы проверят вашу заявку и начислят вам репутацию",
    )
    .reply_markup(InlineKeyboardMarkup::new([[menu_button()]]))
    .await?;

    Ok(())
}

async fn unit_report(
    bot: &Bot,
    db: &OknoId,
    session: &Session,
    drop_id: DropId,
    user_id: UserId,
) -> anyhow::Result<()> {
    check_banned(bot, db, user_id, user_id).await?;

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

    bot.send_message(user_id, "киньте скрин вашего комментария/отзыва")
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Отмена", CANCEL_CALLBACK),
        ]]))
        .await?;

    Ok(())
}

pub async fn on_unit_report_callback(
    bot: Bot,
    db: Arc<OknoId>,
    session: Session,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    bot.answer_callback_query(callback.id.clone()).await?;
    let chat = get_callback_chat(&callback)?;

    check_private(&bot, chat).await?;

    let callback_data = callback.data.ok_or(UtilError::NoCallbackData)?;
    let drop_id = callback_data[UNIT_FEEDBACK_CALLBACK_PREFIX.len()..].parse()?;

    unit_report(&bot, &db, &session, drop_id, callback.from.id).await?;
    Ok(())
}

pub async fn on_unit_report_command(
    bot: Bot,
    db: Arc<OknoId>,
    session: Session,
    message: Message,
) -> anyhow::Result<()> {
    check_private(&bot, &message.chat).await?;

    let Ok(drop_id) = get_args(&message).parse() else {
        invalid_usage_message(&bot, message.chat.id).await?;
        return Ok(());
    };

    let user = message.from.ok_or(UtilError::FailedGetUser)?;

    unit_report(&bot, &db, &session, drop_id, user.id).await?;

    Ok(())
}

pub async fn on_drop_command(bot: Bot, db: Arc<OknoId>, message: Message) -> anyhow::Result<()> {
    let user = message.from.as_ref().ok_or(UtilError::FailedGetUser)?;

    check_user_super_admin(&bot, &db, user.id, message.chat.id).await?;

    let mut args = get_args(&message).split(' ');
    let Some(link) = args.next() else {
        invalid_usage_message(&bot, message.chat.id).await?;
        return Ok(());
    };

    let description = args.next();

    let drop_id = db.add_drop(link, description).await?;
    let text = format!(
        "Новый дроп сообщества!\n{link}{}{}",
        if description.is_some() { "\n> " } else { "" },
        description.unwrap_or("")
    );
    let markup = InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
        "Я оставил фидбек",
        format!("{UNIT_FEEDBACK_CALLBACK_PREFIX}{drop_id}"),
    )]]);

    let mut request = bot.send_message(UserId(0), text).reply_markup(markup);

    for unit in db.get_users_by_role(Role::OknoUnit).await? {
        request.chat_id = unit.into();
        let res = request.send_ref().await;

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

pub async fn on_drops_history_callback(
    bot: Bot,
    db: Arc<OknoId>,
    config: Arc<Config>,
    callback: CallbackQuery,
) -> anyhow::Result<()> {
    let chat_id = callback.chat_id().ok_or(UtilError::FailedGetChat)?;

    let drops = db.get_latest_drops(20).await?;

    let mut drop_completeness = Vec::with_capacity(drops.len());
    for drop in &drops {
        drop_completeness.push(db.check_drop_completed(drop.id, callback.from.id).await?);
    }

    let mut text = "Используйте /feedback &lt;id&gt; для подачи заявки:\n".to_string();
    for (i, drop) in drops.iter().enumerate() {
        writeln!(
            &mut text,
            "ID:{} <a href=\"{}\">ссылка</a>{}",
            drop.id,
            drop.link,
            if drop_completeness[i] {
                "(выполнено)"
            } else {
                ""
            }
        )?;
        if let Some(description) = &drop.description {
            writeln!(&mut text, "> {description}",)?;
        }
    }

    bot.send_photo(chat_id, InputFile::file_id(config.banners.unit.clone()))
        .caption(text)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Назад", UNIT_INFO_CALLBACK.to_owned()),
            menu_button(),
        ]]))
        .await?;

    try_delete_origin(&bot, &callback).await?;
    bot.answer_callback_query(callback.id).await?;
    Ok(())
}
