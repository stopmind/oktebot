use crate::{
    config::Config,
    oknoid::IdError::{InvalidRole, UserExists, UserNotFound},
};
use log::info;
use sqlx::{Error, SqlitePool, sqlite::SqliteConnectOptions};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    fs, io,
    ops::DerefMut,
    sync::{Arc, Mutex},
};
use teloxide::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdError {
    #[error("db error: {0}")]
    DbError(#[from] sqlx::Error),
    #[error("db initialization failed due io error: {0}")]
    InitIoError(io::Error),
    #[error("user already exists: {0}")]
    UserExists(UserId),
    #[error("user not found: {0}")]
    UserNotFound(UserId),
    #[error("invalid role id: {0}")]
    InvalidRole(i64),
}

impl IdError {
    fn map_user_not_found(err: sqlx::Error, id: UserId) -> Self {
        match err {
            Error::RowNotFound => UserNotFound(id),
            err => err.into(),
        }
    }
}

type IdResult<T> = Result<T, IdError>;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default)]
pub enum UserRole {
    #[default]
    Standard = 0,
    Admin = 1,
    SuperAdmin = 2,
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
    const fn from_db(val: i64) -> Option<UserRole> {
        macro_rules! chk {
            ($($i:ident),*) => {
                match val {
                    $(x if x == $i as i64 => Some($i),)*
                    _ => None
                }
            };
        }

        use UserRole::*;
        chk!(Standard, Admin, SuperAdmin)
    }

    const fn to_db(self) -> i64 {
        self as i64
    }
}

#[derive(Default)]
pub struct UserInfo {
    pub role: UserRole,
    pub reputation: i64,
    pub bio: Option<String>,
}

struct Usernames {
    username_to_id: HashMap<String, UserId>,
    id_to_username: HashMap<UserId, String>,
}

pub struct OknoId {
    pool: SqlitePool,
    config: Arc<Config>,
    usernames: Mutex<Usernames>,
}

impl OknoId {
    pub async fn open(config: Arc<Config>) -> IdResult<Self> {
        let path = config.storage.join("id.db");
        let do_initialize = !fs::exists(&path).map_err(IdError::InitIoError)?;

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

        let users = sqlx::query_as::<'_, _, (i64, String)>("SELECT id, username FROM users")
            .fetch_all(&pool)
            .await?;

        for (id, username) in users {
            let id = UserId(id as u64);
            username_to_id.insert(username.clone(), id);
            id_to_username.insert(id, username);
        }

        info!("Id database loaded.");

        Ok(OknoId {
            pool,
            config,
            usernames: Mutex::new(Usernames {
                username_to_id,
                id_to_username,
            }),
        })
    }

    async fn initialize(pool: &SqlitePool) -> IdResult<()> {
        sqlx::raw_sql(
            "
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
            ) WITHOUT ROWID, STRICT;",
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl OknoId {
    pub async fn register_user(
        &self,
        id: UserId,
        username: String,
        info: UserInfo,
    ) -> IdResult<()> {
        {
            let mut usernames = self.usernames.lock().unwrap();
            if usernames.id_to_username.contains_key(&id) {
                return Err(UserExists(id));
            }

            usernames.id_to_username.insert(id, username.clone());
            usernames.username_to_id.insert(username.clone(), id);
        }
        sqlx::query(
            "INSERT INTO users (id, role, reputation, bio, username) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id.0 as i64)
        .bind(info.role.to_db())
        .bind(info.reputation)
        .bind(info.bio)
        .bind(username)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_username(&self, user_id: UserId, username: String) -> IdResult<()> {
        let query = {
            let mut guard = self.usernames.lock().unwrap();

            let usernames = guard.deref_mut();

            if let Some(old_username) = usernames.id_to_username.get_mut(&user_id)
                && old_username != username.as_str()
            {
                let query = sqlx::query("UPDATE users SET username = ? WHERE id = ?")
                    .bind(&username)
                    .bind(user_id.0 as i64);

                usernames.username_to_id.remove(old_username);
                old_username.clear();
                old_username.push_str(&username);
                usernames.username_to_id.insert(username, user_id);

                Some(query)
            } else {
                None
            }
        };

        if let Some(query) = query {
            query.execute(&self.pool).await?;
        }

        Ok(())
    }

    pub fn resolve_username(&self, username: &str) -> Option<UserId> {
        self.usernames
            .lock()
            .unwrap()
            .username_to_id
            .get(username)
            .copied()
    }

    pub fn get_username(&self, id: UserId) -> Option<String> {
        self.usernames
            .lock()
            .unwrap()
            .id_to_username
            .get(&id)
            .cloned()
    }

    pub async fn set_bio(&self, id: UserId, bio: Option<&str>) -> IdResult<()> {
        let affected = sqlx::query("UPDATE users SET bio = ? WHERE id = ?")
            .bind(bio)
            .bind(id.0 as i64)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if affected == 0 {
            Err(UserNotFound(id))
        } else {
            Ok(())
        }
    }

    pub async fn get_user_info(&self, id: UserId) -> IdResult<UserInfo> {
        let (role, reputation, bio): (_, _, Option<String>) =
            sqlx::query_as("SELECT role, reputation, bio FROM users WHERE id = ?")
                .bind(id.0 as i64)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| IdError::map_user_not_found(e, id))?;

        let mut role = UserRole::from_db(role).ok_or_else(|| InvalidRole(role))?;

        if self.config.super_admins.contains(&id) {
            role = UserRole::SuperAdmin;
        }

        Ok(UserInfo {
            role,
            reputation,
            bio,
        })
    }

    pub async fn get_user_role(&self, id: UserId) -> IdResult<UserRole> {
        if self.config.super_admins.contains(&id) {
            return Ok(UserRole::SuperAdmin);
        }

        let (role,) = sqlx::query_as("SELECT role FROM users WHERE id = ?")
            .bind(id.0 as i64)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| IdError::map_user_not_found(e, id))?;

        UserRole::from_db(role).ok_or_else(|| InvalidRole(role))
    }

    pub async fn set_user_role(&self, id: UserId, role: UserRole) -> IdResult<()> {
        let affected = sqlx::query("UPDATE users SET role = ? WHERE id = ?")
            .bind(role.to_db())
            .bind(id.0 as i64)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if affected == 0 {
            Err(UserNotFound(id))
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    fn is_super_admin(&self, id: UserId) -> bool {
        self.config.super_admins.contains(&id)
    }

    pub async fn add_reputation(&self, id: UserId, val: i64) -> IdResult<i64> {
        let result = sqlx::query_as::<'_, _, (i64,)>(
            "\
                UPDATE users \
                SET reputation = reputation + ? \
                WHERE id = ? \
                RETURNING reputation \
                ",
        )
        .bind(val)
        .bind(id.0 as i64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| IdError::map_user_not_found(e, id))?;

        Ok(result.0)
    }
}
