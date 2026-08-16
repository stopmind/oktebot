use anyhow::{anyhow, bail, Result};
use log::{error, info};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{fs, path::Path, sync::Arc};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::DerefMut;
use std::sync::Mutex;
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::types::{ChatKind, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, User};
use crate::scheme::CANCEL_CALLBACK;
use crate::session::{Session, SessionState};
use crate::utils::Mention;

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
    fn from_db(val: i64) -> Option<UserRole> {
        macro_rules! chk {
            ($($i:ident),*) => {
                match val {
                    $(x if x == $i as i64 => Some($i),)*
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

    fn to_db(self) -> i64 {
        self as i64
    }
}

#[derive(Default)]
pub struct UserInfo {
    role: UserRole,
    reputation: i64,
    bio: Option<String>,
}

struct Usernames {
    username_to_id: HashMap<String, UserId>,
    id_to_username: HashMap<UserId, String>,
}

pub struct IdDb {
    pool: SqlitePool,
    usernames: Mutex<Usernames>,
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

        let mut username_to_id = HashMap::new();
        let mut id_to_username = HashMap::new();

        let users =sqlx::query_as::<'_, _, (i64, String)>("SELECT id, username FROM users")
            .fetch_all(&pool)
            .await?;

        for (id, username) in users {
            let id = UserId(id as u64);
            username_to_id.insert(username.clone(), id);
            id_to_username.insert(id, username);
        }

        info!("Id database loaded.");

        Ok(IdDb {
            pool,
            usernames: Mutex::new(Usernames {
                username_to_id,
                id_to_username,
            })
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
                bio TEXT, \
                username TEXT \
                    NOT NULL
            ) WITHOUT ROWID, STRICT;")
            .execute(pool)
            .await?;
        Ok(())
    }
}

impl IdDb {
    async fn register_user(&self, id: UserId, username: String, info: UserInfo) -> Result<()> {
        {
            let mut usernames = self.usernames.lock().unwrap();
            if usernames.id_to_username.contains_key(&id) {
                return Err(anyhow!("User already exists"));
            }

            usernames.id_to_username.insert(id, username.clone());
            usernames.username_to_id.insert(username.clone(), id);
        }
        sqlx::query("INSERT INTO users (id, role, reputation, bio, username) VALUES (?, ?, ?, ?, ?)")
            .bind(id.0 as i64)
            .bind(info.role.to_db())
            .bind(info.reputation)
            .bind(info.bio)
            .bind(username)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn update_username(&self, user_id: UserId, username: String) -> Result<()> {
        let query = {
            let mut guard = self.usernames.lock()
                .map_err(|_| anyhow!("Failed to lock usernames!"))?;

            let usernames = guard.deref_mut();

            if let Some(old_username) = usernames.id_to_username.get_mut(&user_id) &&
                old_username != username.as_str()
            {
                let query = sqlx::query("UPDATE users SET username = ? WHERE id = ?")
                    .bind(&username)
                    .bind(user_id.0 as i64);

                usernames.username_to_id.remove(old_username);
                old_username.clear();
                old_username.push_str(&username);
                usernames.username_to_id.insert(username, user_id);

                Some(query)
            } else { None }
        };

        if let Some(query) = query {
            query.execute(&self.pool).await?;
        }

        Ok(())
    }

    pub fn resolve_username(&self, username: &str) -> Option<UserId> {
        self.usernames.lock().unwrap()
            .username_to_id.get(username).copied()
    }

    pub fn get_username(&self, id: UserId) -> Option<String> {
        self.usernames.lock().unwrap()
            .id_to_username.get(&id).cloned()
    }

    async fn set_bio(&self, id: UserId, bio: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE users SET bio = ? WHERE id = ?")
            .bind(bio)
            .bind(id.0 as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_user_info(&self, id: UserId) -> Result<UserInfo> {
        let (role, reputation, bio): (_, _, Option<String>) =
            sqlx::query_as("SELECT role, reputation, bio FROM users WHERE id = ?")
                .bind(id.0 as i64)
                .fetch_one(&self.pool)
                .await?;

        let role = UserRole::from_db(role)
            .ok_or_else(|| anyhow!("Invalid user role ID: {}", role))?;

        Ok(UserInfo {
            role,
            reputation,
            bio,
        })
    }
}

pub async fn usernames_inspect(
    message: Message,
    db: Arc<IdDb>
) {
    if let Some(User { id, username: Some(username), ..}) = message.from.as_ref() {
        if let Err(err) = db.update_username(*id, username.clone()).await {
            error!("Error while updating username: {:?}", err);
        }
    }
}

pub async fn on_start(
    bot: Bot,
    message: Message,
    db: Arc<IdDb>,
) -> Result<()> {
    let Some(User { id, username: Some(username), ..}) = message.from.as_ref() else {
        bail!("failed get user or username");
    };

    if let Err(error) = db.register_user(*id, username.clone(), UserInfo::default()).await {
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
    db: Arc<IdDb>,
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
            "Пользователь: @{username}\n\
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
    mention: String,
    db: Arc<IdDb>,
) -> Result<()> {
    async fn inner<'m>(mention: &'m str, db: &IdDb) -> Option<(UserId, Cow<'m, str>)> {
        let mention = Mention::parse(mention)?;
        Some(match mention {
            Mention::Username(username) =>
                (db.resolve_username(username)?, Cow::Borrowed(username)),
            Mention::UserId(id) =>
                (id, Cow::Owned(db.get_username(id)?))
        })
    }

    if let Some((id, username)) = inner(&mention, &db).await {
        send_profile(&bot, &db, message.chat.id, id, username.as_ref()).await?;
    } else {
        bot.send_message(message.chat.id,
            "Пользователь не найден. Нужно указывать id или @имя пользователя, зарегестрированного в боте"
        ).await?;
    }

    Ok(())
}

pub async fn on_me(
    bot: Bot,
    message: Message,
    db: Arc<IdDb>,
) -> Result<()> {
    let Some(User { id, username: Some(username), ..}) = message.from.as_ref() else {
        bail!("failed get user or username");
    };

    send_profile(&bot, &db, message.chat.id, *id, username.as_str()).await
}
