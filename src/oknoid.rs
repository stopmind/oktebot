use crate::{
    config::Config,
    oknoid::IdError::{UserExists, UserNotFound},
};
use futures::{TryStreamExt, stream::StreamExt};
use log::info;
use sqlx::{Error, FromRow, SqlitePool, migrate::Migrator, sqlite::SqliteConnectOptions};
use std::{collections::{BTreeSet, HashMap}, fmt::{Display, Formatter}, sync::{Arc, Mutex}};
use std::borrow::Cow;
use std::collections::BTreeMap;
use teloxide::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdError {
    #[error("db error: {0}")]
    DbError(#[from] sqlx::Error),
    #[error("db initialization failed due migration error: {0}")]
    InitMigrateError(#[from] sqlx::migrate::MigrateError),
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Role {
    Admin = 1,
    SuperAdmin = 2,
    OknoUnit = 3,
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Role::Admin => "админ",
            Role::SuperAdmin => "СУПЕРадмин",
            Role::OknoUnit => "OKNO UNIT",
        })
    }
}

impl Role {
    const fn from_db(val: i64) -> IdResult<Role> {
        macro_rules! chk {
            ($($i:ident),*) => {
                match val {
                    $(x if x == $i as i64 => Ok($i),)*
                    x => Err(IdError::InvalidRole(x))
                }
            };
        }

        use Role::*;
        chk!(Admin, OknoUnit)
    }

    const fn to_db(self) -> i64 {
        self as i64
    }
}

#[derive(Default)]
pub struct UserInfo {
    pub username: Option<String>,
    pub first_name: String,
    #[allow(dead_code)]
    pub roles: BTreeSet<Role>,
    #[allow(dead_code)]
    pub is_banned: bool,
    pub reputation: i64,
    pub bio: Option<String>,
}

#[derive(FromRow)]
pub struct DropInfo {
    pub id: DropId,
    pub link: String,
    pub description: Option<String>,
}

#[derive(Default)]
struct UsersNames {
    id_to_names: BTreeMap<UserId, (Arc<str>, Option<Arc<str>>)>,
    username_to_id: HashMap<Arc<str>, UserId>,
    first_name_to_ids: BTreeMap<Arc<str>, BTreeSet<UserId>>,
}

pub struct Names<'s> {
    pub username: Option<Cow<'s, str>>,
    pub last_name: Cow<'s, str>,
}

impl<'s> Display for Names<'s> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(username) = self.username.as_ref() {
            write!(f, "@{}", username)
        } else {
            f.write_str(&self.last_name)
        }
    }
}

impl UsersNames {

    /// returns true if values changed
    fn try_update(
        &mut self,
        user_id: UserId,
        username: Option<&str>,
        first_name: &str
    ) -> bool {
        if let Some((current_first_name, current_username)) = self.id_to_names.get_mut(&user_id) {
            let first_name_changed = current_first_name.as_ref() != first_name;
            let username_changed = current_username.as_deref() != username;

            if first_name_changed {
                let first_name: Arc<str> = Arc::from(first_name);

                let ids = self.first_name_to_ids.get_mut(current_first_name)
                    .expect("if this first name is current then it must be presented in first_name_to_id");

                ids.remove(&user_id);
                if ids.is_empty() {
                    self.first_name_to_ids.remove(current_first_name);
                }
                *current_first_name = first_name.clone();

                self.first_name_to_ids.entry(first_name)
                    .and_modify(|ids| { ids.insert(user_id); })
                    .or_insert_with(|| BTreeSet::from_iter([user_id]));
            }

            if username_changed {
                let username: Option<Arc<str>> = username.map(Arc::from);
                if let Some(current_username) = current_username {
                    self.username_to_id.remove(current_username);
                }

                if let Some(username) = username.clone() {
                    self.username_to_id.insert(username, user_id);
                }

                *current_username = username;
            }

            first_name_changed | username_changed
        } else {
            self.add(user_id, username, first_name);
            true
        }
    }

    fn add(
        &mut self,
        user_id: UserId,
        username: Option<&str>,
        first_name: &str
    ) {
        let username: Option<Arc<str>> = username.map(Arc::from);
        let first_name: Arc<str> = Arc::from(first_name);

        self.id_to_names.insert(user_id, (first_name.clone(), username.clone()));
        self.first_name_to_ids.entry(first_name)
            .and_modify(|ids| { ids.insert(user_id); })
            .or_insert_with(|| BTreeSet::from_iter([user_id]));
        if let Some(username) = username {
            self.username_to_id.insert(username, user_id);
        }
    }

    fn contains_user(&self, user_id: UserId) -> bool {
        self.id_to_names.contains_key(&user_id)
    }

    fn get_names(&self, user_id: UserId) -> Option<Names<'_>> {
        self.id_to_names.get(&user_id)
            .map(|(first_name, username)| {
                Names {
                    username: username.as_deref().map(Cow::Borrowed),
                    last_name: first_name.as_ref().into(),
                }
            })
    }

    fn get_ids_by_first_name(&self, first_name: &str) -> Option<impl Iterator<Item = UserId>> {
        self.first_name_to_ids
            .get(first_name)
            .map(|ids| ids.iter().copied())
    }

    fn get_id_by_username(&self, username: &str) -> Option<UserId> {
        self.username_to_id.get(username)
            .copied()
    }
}

pub struct OknoId {
    pool: SqlitePool,
    config: Arc<Config>,
    users_names: Mutex<UsersNames>,
}

pub type DropId = i64;

//noinspection ALL
static MIGRATOR: Migrator = sqlx::migrate!();

impl OknoId {
    pub async fn open(config: Arc<Config>) -> IdResult<Self> {
        let path = config.storage.join("id.db");
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .create_if_missing(true)
                .filename(path),
        )
        .await?;

        MIGRATOR.run(&pool).await?;

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
            users_names: Default::default(),
        })
    }
}

impl OknoId {
    pub async fn register_user(
        &self,
        id: UserId,
        info: UserInfo,
    ) -> IdResult<()> {
        {
            let mut users = self.users_names.lock().unwrap();
            if users.contains_user(id) {
                return Err(UserExists(id));
            }

            users.add(id, info.username.as_deref(), &info.first_name);
        }

        sqlx::query("INSERT INTO users (id, reputation, bio, username, first_name) VALUES (?, ?, ?, ?, ?)")
            .bind(id.0 as i64)
            .bind(info.reputation)
            .bind(info.bio)
            .bind(info.username)
            .bind(info.first_name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_username(&self, id: UserId, username: Option<&str>, first_name: &str) -> IdResult<()> {
        let updated = {
            let mut users = self.users_names.lock().unwrap();
            users.try_update(id, username, first_name)
        };

        if updated {
            sqlx::query("UPDATE users SET username = ?, first_name = ? WHERE id = ?")
                .bind(id.0 as i64)
                .bind(username)
                .bind(first_name)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub fn resolve_username(&self, username: &str) -> Option<UserId> {
        self.users_names
            .lock()
            .unwrap()
            .get_id_by_username(username)
    }

    pub fn get_user_names(&self, id: UserId) -> Option<Names<'static>> {
        let users = self.users_names
            .lock()
            .unwrap();
        
        users.get_names(id)
            .map(|n| Names {
                username: n.username.map(|s| s.into_owned().into()),
                last_name: n.last_name.into_owned().into(),
            })
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
        let (username, first_name, reputation, bio, is_banned) =
            sqlx::query_as("SELECT username, first_name, reputation, bio, banned FROM users WHERE id = ?")
                .bind(id.0 as i64)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| IdError::map_user_not_found(e, id))?;

        Ok(UserInfo {
            username,
            first_name,
            roles: self.get_roles(id).await?,
            reputation,
            bio,
            is_banned,
        })
    }

    pub fn is_super_admin(&self, id: UserId) -> bool {
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

    pub async fn get_roles(&self, id: UserId) -> IdResult<BTreeSet<Role>> {
        let raw_ids = sqlx::query_as("SELECT role FROM users_roles WHERE user_id = ?")
            .bind(id.0 as i64)
            .fetch_all(&self.pool)
            .await?;

        let mut roles = <IdResult<BTreeSet<_>>>::from_iter(
            raw_ids.into_iter().map(|r: (_,)| Role::from_db(r.0)),
        )?;

        if self.is_super_admin(id) {
            roles.insert(Role::SuperAdmin);
        }

        Ok(roles)
    }

    /// returns true if user didn't have role
    pub async fn give_role(&self, id: UserId, role: Role) -> IdResult<bool> {
        Ok(
            sqlx::query("INSERT OR IGNORE INTO users_roles (user_id, role) VALUES (?, ?)")
                .bind(id.0 as i64)
                .bind(role.to_db())
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }
    /// returns true if user did have role
    pub async fn take_role(&self, id: UserId, role: Role) -> IdResult<bool> {
        Ok(
            sqlx::query("DELETE FROM users_roles WHERE user_id = ? AND role = ?")
                .bind(id.0 as i64)
                .bind(role.to_db())
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }
    pub async fn check_role(&self, id: UserId, role: Role) -> IdResult<bool> {
        let (has_role,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM users_roles WHERE user_id = ? AND role = ?)",
        )
        .bind(id.0 as i64)
        .bind(role.to_db())
        .fetch_one(&self.pool)
        .await?;

        Ok(has_role)
    }

    pub async fn check_user_privileges(&self, id: UserId) -> IdResult<bool> {
        Ok(self.is_super_admin(id) || self.check_role(id, Role::Admin).await?)
    }

    pub async fn get_top(&self, offset: u32, limit: u32) -> IdResult<Vec<(UserId, i64, String)>> {
        sqlx::query_as(
            "SELECT id, reputation, username FROM users ORDER BY reputation DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch(&self.pool)
        .map(|res| {
            let (id, rep, username) = res?;
            Ok((UserId(id), rep, username))
        })
        .try_collect()
        .await
    }

    pub async fn get_users_by_role(&self, role: Role) -> IdResult<Vec<UserId>> {
        sqlx::query_as::<_, (u64,)>("SELECT user_id FROM users_roles WHERE role = ?")
            .bind(role.to_db())
            .fetch(&self.pool)
            .map(|id| Ok(UserId(id?.0)))
            .try_collect()
            .await
    }
    pub async fn get_latest_drops(&self, limit: u32) -> IdResult<Vec<DropInfo>> {
        sqlx::query_as("SELECT id, link, description FROM drops ORDER BY id DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(IdError::from)
    }
    pub async fn add_drop(&self, link: &str, description: Option<&str>) -> IdResult<DropId> {
        sqlx::query_as("INSERT INTO drops (link, description) VALUES (?, ?) RETURNING id")
            .bind(link)
            .bind(description)
            .fetch_one(&self.pool)
            .await
            .map(|(id,)| id)
            .map_err(IdError::from)
    }
    pub async fn get_drop(&self, id: DropId) -> IdResult<DropInfo> {
        sqlx::query_as("SELECT id, link, description FROM drops WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(IdError::from)
    }

    /// returns true if drop hadn't been completed by user
    pub async fn mark_drop_completed(&self, id: DropId, user: UserId) -> IdResult<bool> {
        sqlx::query("INSERT OR IGNORE INTO drops_accepted (user_id, drop_id) VALUES (?, ?)")
            .bind(user.0 as i64)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|res| res.rows_affected() == 1)
            .map_err(IdError::from)
    }
    pub async fn check_drop_completed(&self, id: DropId, user: UserId) -> IdResult<bool> {
        sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM drops_accepted WHERE user_id = ? AND drop_id = ?)",
        )
        .bind(user.0 as i64)
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map(|(completed,)| completed)
        .map_err(IdError::from)
    }

    pub async fn check_drop_exists(&self, id: DropId) -> IdResult<bool> {
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM drops WHERE id = ?)")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map(|(exists,)| exists)
            .map_err(IdError::from)
    }

    pub async fn users_count(&self) -> IdResult<u32> {
        sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map(|(count,)| count)
            .map_err(IdError::from)
    }

    pub fn check_user_exists(&self, user: UserId) -> bool {
        self.users_names
            .lock()
            .unwrap()
            .contains_user(user)
    }

    pub async fn is_user_banned(&self, id: UserId) -> IdResult<bool> {
        let (is_banned,) = sqlx::query_as("SELECT banned FROM users WHERE id = ?")
            .bind(id.0 as i64)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| IdError::map_user_not_found(e, id))?;

        Ok(is_banned)
    }

    pub async fn set_user_banned(&self, id: UserId, banned: bool) -> IdResult<()> {
        sqlx::query("UPDATE users SET banned = ? WHERE id = ?")
            .bind(banned)
            .bind(id.0 as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub fn get_ids_by_first_name(&self, first_name: &str) -> Vec<UserId> {
        self.users_names.lock().unwrap()
            .get_ids_by_first_name(first_name)
            .map(Vec::from_iter)
            .unwrap_or(Vec::new())
    }
}
