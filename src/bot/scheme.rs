use crate::{bot::{
    oknounit::*,
    on_cancel_callback, on_help_callback, on_help_command, on_main_menu_callback,
    on_main_menu_command,
    profile::*,
    router::{HandlerOptions, PrivilegeLevel, Router},
    support::*,
}, states, w};
use crate::bot::session::SessionState;

pub const CANCEL_CALLBACK: &str = "cancel";
pub const HELP_CALLBACK: &str = "help";
pub const BIO_CALLBACK: &str = "bio";
pub const PROFILE_CALLBACK_PREFIX: &str = "profile";
pub const SUPPORT_SELECTED_CALLBACK_PREFIX: &str = "support-selected";
pub const UNIT_JOIN_CALLBACK: &str = "unit-join";
pub const UNIT_REPORT_CALLBACK_PREFIX: &str = "unit-feedback";
pub const UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX: &str = "unit-accept";
pub const DROPS_HISTORY_CALLBACK: &str = "drops-history";
pub const MENU_CALLBACK: &str = "menu";
pub const SUPPORT_CALLBACK: &str = "support";
pub const ME_CALLBACK: &str = "me";
pub const UNIT_INFO_CALLBACK: &str = "unit-info";
pub const TOP_CALLBACK_PREFIX: &str = "top";
pub const TOP_CALLBACK: &str = "top:0";

#[rustfmt::skip]
pub fn scheme() -> Router {
    let mut router = Router::default();

    router
        .state(states!{
            SessionState::WaitUnitReport { drop_id } => on_unit_report_message([user], drop_id),
            SessionState::WaitBioMessage => on_bio_message([user]),
            SessionState::WaitSupportMessage { category } => on_support_message([user], category)
        })
        .command("menu", w!(on_main_menu_command), HandlerOptions::empty()
            .only_private(true))
        .command("support", w!(on_support_command), HandlerOptions::empty()
            .only_private(true))
        .command("bio", w!(on_bio_command), HandlerOptions::empty()
            .only_private(true))
        .command("help", w!(on_help_command), HandlerOptions::empty())
        .command("info", w!(on_info_command), HandlerOptions::empty())
        .command("me", w!(on_me_command), HandlerOptions::empty())
        .command("add_admin", w!(on_add_admin_command), HandlerOptions::empty())
        .command("del_admin", w!(on_del_admin), HandlerOptions::empty())
        .command("rep", w!(on_change_rep), HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))
        .command("top", w!(on_top_command), HandlerOptions::empty())
        .command("unit", w!(on_unit_info_command), HandlerOptions::empty())
        .command("feedback", w!(on_unit_report_command), HandlerOptions::empty()
            .only_private(true))
        .command("drop", w!(on_drop_command), HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::SuperAdmin))
        .command("ban", w!(on_ban_command), HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))
        .command("unban", w!(on_unban_command), HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))

        .callback(CANCEL_CALLBACK, w!(on_cancel_callback), HandlerOptions::empty())
        .callback(HELP_CALLBACK, w!(on_help_callback), HandlerOptions::empty())
        .callback(BIO_CALLBACK, w!(on_bio_callback), HandlerOptions::empty()
            .only_private(true))
        .callback(MENU_CALLBACK, w!(on_main_menu_callback),HandlerOptions::empty()
            .remove_old_message(true))
        .callback(UNIT_INFO_CALLBACK, w!(on_unit_info_callback), HandlerOptions::empty()
            .remove_old_message(true))
        .callback(ME_CALLBACK, w!(on_me_callback), HandlerOptions::empty()
            .remove_old_message(true))
        .callback(SUPPORT_CALLBACK, w!(on_support_callback), HandlerOptions::empty()
            .check_blacklist(true)
            .remove_old_message(true))
        .callback(UNIT_JOIN_CALLBACK, w!(on_unit_join_callback), HandlerOptions::empty())
        .callback(DROPS_HISTORY_CALLBACK, w!(on_drops_history_callback), HandlerOptions::empty()
            .remove_old_message(true))
        .callback(PROFILE_CALLBACK_PREFIX, w!(on_profile_callback), HandlerOptions::empty())
        .callback(SUPPORT_SELECTED_CALLBACK_PREFIX, w!(on_support_selected_callback), HandlerOptions::empty()
            .check_blacklist(true))
        .callback(UNIT_REPORT_CALLBACK_PREFIX, w!(on_unit_report_callback), HandlerOptions::empty()
            .only_private(true)
            .check_blacklist(true))
        .callback(UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, w!(on_unit_accept_report_callback), HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))
        .callback(TOP_CALLBACK_PREFIX, w!(on_top_callback), HandlerOptions::empty()
            .remove_old_message(true));

    router
}
