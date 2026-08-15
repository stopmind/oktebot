use anyhow::{anyhow, bail, Result};
use log::{error, info};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{fs, path::Path, sync::Arc};
use std::fmt::{Display, Formatter};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::types::{ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, User};
use crate::scheme::CANCEL_CALLBACK;
use crate::session::{Session, SessionState};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default)]
pub enum UserRole {
    #[default]
    Standard = 0,
    Admin = 1,
    SuperAdmin = 2
}

impl Display for UserRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            UserRole::Standard => "стандартная",
            UserRole::Admin => "админ",
            UserRole::SuperAdmin => "СУПЕРадмин",
        })
    }
}

impl UserRole {
    fn from_i32(val: i32) -> Option<UserRole> {
        macro_rules! chk {
            ($($i:ident),*) => {
                match val {
                    $(x if x == $i as i32 => Some($i),)*
                    _ => None
                }
            };
        }

        use UserRole::*;
        chk!(
            Standard,
            Admin,
            SuperAdmin
        )
    }
}

#[derive(Default)]
pub struct UserInfo {
    role: UserRole,
    reputation: i32,
    bio: Option<String>,
}

#[derive(Clone)]
pub struct IdDb {
    pool: Arc<SqlitePool>,
}

impl IdDb {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let do_initialize = !fs::exists(path)?;

        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .create_if_missing(true)
                .filename(path),
        )
        .await?;

        if do_initialize {
            info!("Creating id database...");
            Self::initialize(&pool).await?;
        };

        info!("Id database loaded.");

        Ok(IdDb {
            pool: Arc::new(pool),
        })
    }

    async fn initialize(pool: &SqlitePool) -> Result<()> {
        sqlx::raw_sql("
            CREATE TABLE users ( \
                id INTEGER \
                    PRIMARY KEY NOT NULL UNIQUE, \
                role INTEGER \
                    NOT NULL, \
                reputation INTEGER \
                    NOT NULL, \
                bio TEXT \
            ) WITHOUT ROWID, STRICT;")
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl IdDb {
    async fn register_user(&self, id: UserId, info: UserInfo) -> Result<()> {
        sqlx::query("INSERT INTO users (id, role, reputation, bio) VALUES (?, ?, ?, ?)")
            .bind(id.0 as i32)
            .bind(info.role as i32)
            .bind(info.reputation)
            .bind(info.bio)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    async fn set_bio(&self, id: UserId, bio: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE users SET bio = ? WHERE id = ?")
            .bind(bio)
            .bind(id.0 as i32)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    async fn get_user_info(&self, id: UserId) -> Result<UserInfo> {
        let (role, reputation, bio): (_, _, Option<String>) =
            sqlx::query_as("SELECT role, reputation, bio FROM users WHERE id = ?")
                .bind(id.0 as i32)
                .fetch_one(self.pool.as_ref())
                .await?;

        let role = UserRole::from_i32(role)
            .ok_or_else(|| anyhow!("Invalid user role ID: {}", role))?;

        Ok(UserInfo {
            role,
            reputation,
            bio,
        })
    }
}


pub async fn on_start(
    bot: Bot,
    message: Message,
    db: IdDb,
) -> Result<()> {
    let user = message.from.as_ref()
        .ok_or(anyhow!("Failed to retrieve user"))?;

    if let Err(error) = db.register_user(user.id, UserInfo::default()).await {
        error!("Error registering user: {:?}", error);
    };

    bot.send_message(message.chat.id, "НЕ ЗАБУДЬ ПЕРЕДЕЛАТЬ ЭТО СООБЩЕНИЕ").await?;
    Ok(())
}

pub async fn on_bio(
    bot: Bot,
    session: Session,
    message: Message
) -> Result<()> {
    if matches!(message.chat.kind, ChatKind::Private(..)) {
        session.update(SessionState::WaitBioMessage).await?;

        bot.send_message(message.chat.id, "Отправьте описание для профиля следующим сообщением.")
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

pub async fn on_bio_message(
    bot: Bot,
    session: Session,
    message: Message,
    db: IdDb,
) -> Result<()> {
    let Some(text) = message.text() else {
        bot.send_message(message.chat.id, "Отправьте сообщение с текстом!").await?;
        return Ok(());
    };

    if text.trim().is_empty() {
        bot.send_message(message.chat.id, "Описание не может быть пустым! >:(").await?;
        return Ok(());
    }

    let user = message.from.as_ref()
        .ok_or(anyhow!("Failed to retrieve user"))?;

    db.set_bio(user.id, Some(text)).await?;

    bot.send_message(message.chat.id, "Описание профиля обновлено!").await?;
    session.exit().await?;
    Ok(())
}

pub async fn on_bio_cancel(
    bot: Bot,
    query: CallbackQuery,
    session: Session
) -> Result<()> {
    session.exit().await?;
    bot.send_message(session.chat_id(), "Отменено.").await?;
    bot.answer_callback_query(query.id).await?;
    Ok(())
}

async fn send_profile(
    bot: &Bot,
    db: &IdDb,
    chat_id: ChatId,
    user_id: UserId,
    username: &str
) -> Result<()> {
    let info = db.get_user_info(user_id).await?;
    let text = if let Some(bio) = info.bio.as_ref() {
        format!(
            "Пользователь: {username}\n\
            Роль: {}\n\
            Репутация: {}.\n\
            Описание:\n\
            {bio}",
            info.role, info.reputation
        )
    } else {
        format!(
            "Пользователь: {username}\n\
            Роль: {}\n\
            Репутация: {}.\n\
            Нет описания.",
            info.role,
            info.reputation
        )
    };
    bot.send_message(chat_id, text).await?;
    Ok(())
}

pub async fn on_info(
    bot: Bot,
    message: Message,
    db: IdDb,
) -> Result<()> {
    todo!()
}

pub async fn on_me(
    bot: Bot,
    message: Message,
    db: IdDb,
) -> Result<()> {
    let Some(User { id, username: Some(username), ..}) = message.from.as_ref() else {
        bail!("failed get user or username");
    };

    send_profile(&bot, &db, message.chat.id, id.clone(), username.as_str()).await
}
