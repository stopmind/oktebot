use crate::bot::scheme::DROPS_HISTORY_CALLBACK_PREFIX;
use crate::{bot::{
    args::get_args,
    invalid_usage_message,
    router::{CallbackInfo, CommandInfo},
    scheme::{
        CANCEL_CALLBACK, DROPS_HISTORY_CALLBACK, MENU_CALLBACK, PROFILE_CALLBACK_PREFIX,
        UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, UNIT_INFO_CALLBACK, UNIT_JOIN_CALLBACK,
        UNIT_REPORT_CALLBACK_PREFIX,
    },
    session::{SessionState, Sessions},
    support,
    support::SupportCategory,
    utils::{menu_button, UtilError},
    BotContext,
}, config::Config, fmt_callback, oknoid::{DropId, OknoId, Role}};
use log::error;
use std::{fmt::Write, iter};
use teloxide::{
    payloads::{EditMessageReplyMarkupSetters, SendMessageSetters, SendPhotoSetters},
    requests::{Request, Requester},
    types::{
        ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile,
        Message, ParseMode, UserId,
    },
    Bot,
};
use teloxide::types::User;

async fn unit_info(
    bot: &Bot,
    db: &OknoId,
    config: &Config,
    user_id: UserId,
    chat_id: ChatId,
    from_menu: bool,
) -> anyhow::Result<()> {
    if db.check_role(user_id, Role::OknoUnit).await? {
        let drops = db.get_latest_drops(0, 3).await?;

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
                        fmt_callback![UNIT_REPORT_CALLBACK_PREFIX, drop.id]
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

        bot.send_photo(chat_id, InputFile::url(config.banners.unit.clone()))
            .caption(text)
            .reply_markup(markup)
            .await?;
    } else {
        bot.send_photo(chat_id,  InputFile::url(config.banners.unit.clone()))
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
    ctx: &BotContext,
    info: &CommandInfo,
    _args: (),
) -> anyhow::Result<()> {
    unit_info(
        &ctx.bot,
        &ctx.db,
        &ctx.config,
        info.from.id,
        info.chat.id,
        false,
    )
    .await
}

pub async fn on_unit_info_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    _: (),
) -> anyhow::Result<()> {
    let chat_id = info.message.chat().id;
    unit_info(&ctx.bot, &ctx.db, &ctx.config, info.from.id, chat_id, true).await?;
    Ok(())
}

pub async fn on_unit_join_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    _: (),
) -> anyhow::Result<()> {
    let chat_id = info.message.chat().id;
    if ctx.db.give_role(info.from.id, Role::OknoUnit).await? {
        ctx.bot
            .send_message(chat_id, "Вы стали OknoUnit!")
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::callback("В меню", MENU_CALLBACK),
            ]]))
            .await?;
    }
    Ok(())
}

pub async fn on_unit_accept_report_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    (unit_id, drop_id, rep_count): (u64, DropId, i64),
) -> anyhow::Result<()> {
    let chat = info.message.chat();
    let message_id = info.message.id();

    let unit_id = UserId(unit_id);
    let user_names = ctx
        .db
        .get_user_names(unit_id)
        .ok_or(UtilError::FailedGetUser)?;

    if ctx.db.mark_drop_completed(drop_id, unit_id).await? {
        let new_rep = ctx.db.add_reputation(unit_id, rep_count).await?;
        ctx.bot.send_message(
            chat.id,
            format!("Дроп был отмечен как выполненный для {user_names}.\nОбновленная репутация: {new_rep}"),
        )
        .await?;
        ctx.bot
            .send_message(
                unit_id,
                format!("Ваша заявка по дропу {drop_id} была принята."),
            )
            .await?;

        ctx.bot
            .edit_message_reply_markup(chat.id, message_id)
            .reply_markup(InlineKeyboardMarkup::new([[
                InlineKeyboardButton::callback(
                    "Описание профиля",
                    fmt_callback![PROFILE_CALLBACK_PREFIX, unit_id],
                ),
            ]]))
            .await?;
    } else {
        ctx.bot
            .send_message(chat.id, "Дроп уже был выполнен данным пользователем.")
            .await?;
    }
    Ok(())
}

pub async fn on_unit_report_message(
    ctx: &BotContext,
    message: &Message,
    (user, drop_id,): (&User, DropId),
) -> anyhow::Result<()> {
    let drop = ctx.db.get_drop(drop_id).await?;

    ctx.bot.forward_message(ctx.config.support_chat, message.chat.id, message.id)
        .await?;
    ctx.bot.send_message(
        ctx.config.support_chat,
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
            fmt_callback![PROFILE_CALLBACK_PREFIX, user.id],
        )],
        [InlineKeyboardButton::callback(
            "Подтвердить +1",
            fmt_callback![UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, user.id, drop_id, 1],
        )],
        [InlineKeyboardButton::callback(
            "Подтвердить +2",
            fmt_callback![UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, user.id, drop_id, 2],
        )],
        [InlineKeyboardButton::callback(
            "Подтвердить +3",
            fmt_callback![UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, user.id, drop_id, 3],
        )],
    ]))
    .await?;

    ctx.bot.send_message(
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
    sessions: &Sessions,
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

    sessions.set(user_id, SessionState::WaitUnitReport { drop_id });

    bot.send_message(user_id, "киньте скрин вашего комментария/отзыва")
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::callback("Отмена", CANCEL_CALLBACK),
        ]]))
        .await?;

    Ok(())
}

pub async fn on_unit_report_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    (drop_id,): (DropId,),
) -> anyhow::Result<()> {
    unit_report(&ctx.bot, &ctx.db, &ctx.sessions, drop_id, info.from.id).await?;
    Ok(())
}

pub async fn on_unit_report_command(
    ctx: &BotContext,
    info: &CommandInfo,
    (drop_id,): (DropId,),
) -> anyhow::Result<()> {
    unit_report(&ctx.bot, &ctx.db, &ctx.sessions, drop_id, info.from.id).await?;
    Ok(())
}

pub async fn on_drop_command(
    ctx: &BotContext,
    info: &CommandInfo,
    _args: (),
) -> anyhow::Result<()> {
    let args = get_args(info.text.as_str());
    let (link, description) = if let Some((link, description)) = args.split_once(' ') {
        (link, Some(description))
    } else {
        (args, None)
    };

    if link.is_empty() {
        invalid_usage_message(&ctx.bot, info.chat.id).await?;
        return Ok(());
    }

    let drop_id = ctx.db.add_drop(link, description).await?;
    let text = format!(
        "Новый дроп сообщества!\n{link}{}{}",
        if description.is_some() { "\n> " } else { "" },
        description.unwrap_or("")
    );
    let markup = InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(
        "Я оставил фидбек",
        fmt_callback![UNIT_REPORT_CALLBACK_PREFIX, drop_id]
    )]]);

    let mut request = ctx.bot.send_message(UserId(0), text).reply_markup(markup);

    for unit in ctx.db.get_users_by_role(Role::OknoUnit).await? {
        request.chat_id = unit.into();
        let res = request.send_ref().await;

        if let Err(err) = res {
            error!("Failed to notify user {unit} about drop {drop_id} due: {err}");
        }
    }

    ctx.bot
        .send_message(info.chat.id, format!("Новый дроп создан с id: {drop_id}"))
        .await?;

    Ok(())
}

pub async fn on_drops_history_callback(
    ctx: &BotContext,
    info: &CallbackInfo,
    (page,): (u32,),
) -> anyhow::Result<()> {
    let chat_id = info.message.chat().id;

    const PAGE_SIZE: u32 = 20;

    let pages_count = ctx.db.get_drops_count().await?.div_ceil(PAGE_SIZE);
    let drops = ctx.db.get_latest_drops(page * PAGE_SIZE, PAGE_SIZE).await?;

    let mut drop_completeness = Vec::with_capacity(drops.len());
    for drop in &drops {
        drop_completeness.push(ctx.db.check_drop_completed(drop.id, info.from.id).await?);
    }

    let mut text = format!(
        "Страница ({}/{pages_count}). Используйте /feedback &lt;id&gt; для подачи заявки:\n",
        page + 1
    );
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

    let markup = InlineKeyboardMarkup::new(
        [
            Some([InlineKeyboardButton::callback(
                "Назад",
                UNIT_INFO_CALLBACK.to_owned(),
            )]),
            (page > 0).then(|| {
                [InlineKeyboardButton::callback(
                    "< Предыдущая страница",
                    format!("{DROPS_HISTORY_CALLBACK_PREFIX}{}", page - 1),
                )]
            }),
            (page + 1 < pages_count).then(|| {
                [InlineKeyboardButton::callback(
                    "Следующая страница >",
                    format!("{DROPS_HISTORY_CALLBACK_PREFIX}{}", page + 1),
                )]
            }),
            Some([menu_button()]),
        ]
        .into_iter()
        .flatten(),
    );

    ctx.bot.send_photo(chat_id, InputFile::url(ctx.config.banners.unit.clone()))
        .caption(text)
        .parse_mode(ParseMode::Html)
        .reply_markup(markup)
        .await?;
    Ok(())
}
