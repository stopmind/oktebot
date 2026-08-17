use crate::oknoid::OknoId;
use teloxide::types::{CallbackQuery, UserId};

/// Requires CallbackQuery
pub fn callback_filter(
    id: impl AsRef<str> + Send + Sync + 'static,
) -> impl Fn(CallbackQuery) -> bool + Send + Sync + 'static {
    move |query: CallbackQuery| matches!(query.data, Some(query_id) if query_id == id.as_ref())
}

#[derive(Clone, Copy)]
pub enum Mention<'s> {
    Username(&'s str),
    UserId(UserId),
}

impl<'s> Mention<'s> {
    pub fn parse(val: &'s str) -> Option<Self> {
        if let Some(username) = val.strip_prefix("@") {
            Some(Mention::Username(username))
        } else {
            val.parse().ok().map(UserId).map(Mention::UserId)
        }
    }

    pub fn resolve(self, db: &OknoId) -> Option<UserId> {
        match self {
            Mention::Username(username) => db.resolve_username(username),
            Mention::UserId(id) => Some(id),
        }
    }
}
