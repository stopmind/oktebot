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
        .command(w!(on_main_menu_command), "menu", HandlerOptions::empty()
            .only_private(true))
        .command(w!(on_support_command), "support", HandlerOptions::empty()
            .only_private(true))
        .command(w!(on_bio_command), "bio", HandlerOptions::empty()
            .only_private(true))
        .command(w!(on_help_command), "help", HandlerOptions::empty())
        .command(w!(on_info_command), "info", HandlerOptions::empty())
        .command(w!(on_me_command), "me", HandlerOptions::empty())
        .command(w!(on_add_admin_command), "add_admin", HandlerOptions::empty())
        .command(w!(on_del_admin), "del_admin", HandlerOptions::empty())
        .command(w!(on_change_rep), "rep", HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))
        .command(w!(on_top_command), "top", HandlerOptions::empty())
        .command(w!(on_unit_info_command), "unit", HandlerOptions::empty())
        .command(w!(on_unit_report_command), "feedback", HandlerOptions::empty()
            .only_private(true))
        .command(w!(on_drop_command), "drop", HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::SuperAdmin))
        .command(w!(on_ban_command), "ban", HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))
        .command(w!(on_unban_command), "unban", HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))

        .callback(w!(on_cancel_callback), CANCEL_CALLBACK, HandlerOptions::empty())
        .callback(w!(on_help_callback), HELP_CALLBACK, HandlerOptions::empty())
        .callback(w!(on_bio_callback), BIO_CALLBACK, HandlerOptions::empty()
            .only_private(true))
        .callback(w!(on_main_menu_callback),MENU_CALLBACK,HandlerOptions::empty()
            .remove_old_message(true))
        .callback(w!(on_unit_info_callback), UNIT_INFO_CALLBACK, HandlerOptions::empty()
            .remove_old_message(true))
        .callback(w!(on_me_callback), ME_CALLBACK, HandlerOptions::empty()
            .remove_old_message(true))
        .callback(w!(on_support_callback), SUPPORT_CALLBACK, HandlerOptions::empty()
            .check_blacklist(true)
            .remove_old_message(true))
        .callback(w!(on_unit_join_callback), UNIT_JOIN_CALLBACK, HandlerOptions::empty())
        .callback(w!(on_drops_history_callback), DROPS_HISTORY_CALLBACK, HandlerOptions::empty()
            .remove_old_message(true))
        .callback(w!(on_profile_callback), PROFILE_CALLBACK_PREFIX, HandlerOptions::empty())
        .callback(w!(on_support_selected_callback), SUPPORT_SELECTED_CALLBACK_PREFIX, HandlerOptions::empty()
            .check_blacklist(true))
        .callback(w!(on_unit_report_callback), UNIT_REPORT_CALLBACK_PREFIX, HandlerOptions::empty()
            .only_private(true)
            .check_blacklist(true))
        .callback(w!(on_unit_accept_report_callback), UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX, HandlerOptions::empty()
            .required_privilege(PrivilegeLevel::Admin))
        .callback(w!(on_top_callback), TOP_CALLBACK_PREFIX, HandlerOptions::empty()
            .remove_old_message(true));

    router
}
