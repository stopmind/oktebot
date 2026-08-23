use crate::bot::scheme::MENU_CALLBACK;
use teloxide::types::{CallbackQuery, InlineKeyboardButton, LinkPreviewOptions};

pub const DISABLE_PREVIEW_OPTIONS: LinkPreviewOptions = LinkPreviewOptions {
    is_disabled: true,
    url: None,
    prefer_small_media: false,
    prefer_large_media: false,
    show_above_text: false,
};

pub fn menu_button() -> InlineKeyboardButton {
    InlineKeyboardButton::callback("В меню", MENU_CALLBACK)
}

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
