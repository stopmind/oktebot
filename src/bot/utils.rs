use teloxide::types::CallbackQuery;

/// Requires CallbackQuery
pub fn callback_filter(
    id: impl AsRef<str> + Send + Sync + 'static,
) -> impl Fn(CallbackQuery) -> bool + Send + Sync + 'static {
    move |query: CallbackQuery| matches!(query.data, Some(query_id) if query_id == id.as_ref())
}

/// Requires CallbackQuery
pub fn callback_prefix_filter(
    id: impl AsRef<str> + Send + Sync + 'static,
) -> impl Fn(CallbackQuery) -> bool + Send + Sync + 'static {
    move |query: CallbackQuery| matches!(query.data, Some(query_id) if query_id.starts_with(id.as_ref()))
}
