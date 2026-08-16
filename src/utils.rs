use teloxide::types::{CallbackQuery, UserId};

/// Requires CallbackQuery
pub fn callback_filter(
    id: impl AsRef<str> + Send + Sync + 'static
) -> impl Fn(CallbackQuery) -> bool + Send + Sync + 'static {
    move |query: CallbackQuery| {
        matches!(query.data, Some(query_id) if query_id == id.as_ref())
    }
}

#[derive(Clone, Copy)]
pub enum Mention<'s> {
    Username(&'s str),
    UserId(UserId),
}

impl<'s> Mention<'s> {
    pub fn parse(val: &'s str) -> Option<Self> {
        if val.starts_with("@") {
            Some(Mention::Username(&val[1..]))
        } else {
            val.parse().ok()
                .map(UserId)
                .map(Mention::UserId)
        }
    }
}