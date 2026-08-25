use crate::bot::{
    command::Command,
    oknounit::{
        on_drop_command, on_drops_history_callback, on_unit_accept_report_callback,
        on_unit_info_callback, on_unit_info_command, on_unit_join_callback,
        on_unit_report_callback, on_unit_report_command, on_unit_report_message,
    },
    on_cancel_callback, on_help_callback, on_help_command, on_main_menu_callback,
    on_main_menu_command,
    profile::{
        on_add_admin_command, on_bio_callback, on_bio_command, on_bio_message, on_change_rep,
        on_del_admin, on_info_command, on_me_callback, on_me_command, on_profile_callback,
        on_start, on_top_callback, on_top_command, usernames_inspect,
    },
    session::SessionState,
    support::*,
    utils,
};
use teloxide::{
    dispatching::{UpdateHandler, dialogue, dialogue::InMemStorage},
    dptree::{case, filter},
    filter_command,
    prelude::*,
};

pub const CANCEL_CALLBACK: &str = "cancel";
pub const HELP_CALLBACK: &str = "help";
pub const BIO_CALLBACK: &str = "bio";
pub const PROFILE_CALLBACK_PREFIX: &str = "profile";
pub const SUPPORT_SELECTED_CALLBACK_PREFIX: &str = "support-selected";
pub const UNIT_JOIN_CALLBACK: &str = "unit-join";
pub const UNIT_FEEDBACK_CALLBACK_PREFIX: &str = "unit-feedback";
pub const UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX: &str = "unit-accept";
pub const DROPS_HISTORY_CALLBACK: &str = "drops-history";
pub const MENU_CALLBACK: &str = "menu";
pub const SUPPORT_CALLBACK: &str = "support";
pub const ME_CALLBACK: &str = "me";
pub const UNIT_INFO_CALLBACK: &str = "unit-info";
pub const TOP_CALLBACK_PREFIX: &str = "top";
pub const TOP_CALLBACK: &str = "top0";

pub fn scheme() -> UpdateHandler<anyhow::Error> {
    dialogue::enter::<Update, InMemStorage<SessionState>, SessionState, _>()
        .branch(
            Update::filter_message()
                .inspect_async(usernames_inspect)
                .branch(
                    filter_command::<Command, _>()
                        .branch(case![Command::Start].endpoint(on_start))
                        .branch(case![Command::Help].endpoint(on_help_command))
                        .branch(case![Command::Support].endpoint(on_support_command))
                        .branch(case![Command::Bio].endpoint(on_bio_command))
                        .branch(case![Command::Info].endpoint(on_info_command))
                        .branch(case![Command::Me].endpoint(on_me_command))
                        .branch(case![Command::AdminAdd].endpoint(on_add_admin_command))
                        .branch(case![Command::AdminDel].endpoint(on_del_admin))
                        .branch(case![Command::Rep].endpoint(on_change_rep))
                        .branch(case![Command::Top].endpoint(on_top_command))
                        .branch(case![Command::Unit].endpoint(on_unit_info_command))
                        .branch(case![Command::Feedback].endpoint(on_unit_report_command))
                        .branch(case![Command::Drop].endpoint(on_drop_command))
                        .branch(case![Command::Menu].endpoint(on_main_menu_command)),
                )
                .branch(
                    case![SessionState::WaitSupportMessage { category }]
                        .endpoint(on_support_message),
                )
                .branch(case![SessionState::WaitBioMessage].endpoint(on_bio_message))
                .branch(
                    case![SessionState::WaitUnitReport { drop_id }]
                        .endpoint(on_unit_report_message),
                ),
        )
        .branch(
            Update::filter_callback_query()
                .branch(
                    filter(utils::callback_filter(CANCEL_CALLBACK)).endpoint(on_cancel_callback),
                )
                .branch(filter(utils::callback_filter(HELP_CALLBACK)).endpoint(on_help_callback))
                .branch(filter(utils::callback_filter(BIO_CALLBACK)).endpoint(on_bio_callback))
                .branch(
                    filter(utils::callback_filter(MENU_CALLBACK)).endpoint(on_main_menu_callback),
                )
                .branch(
                    filter(utils::callback_filter(UNIT_INFO_CALLBACK))
                        .endpoint(on_unit_info_callback),
                )
                .branch(filter(utils::callback_filter(ME_CALLBACK)).endpoint(on_me_callback))
                .branch(
                    filter(utils::callback_filter(SUPPORT_CALLBACK)).endpoint(on_support_callback),
                )
                .branch(
                    filter(utils::callback_filter(UNIT_JOIN_CALLBACK))
                        .endpoint(on_unit_join_callback),
                )
                .branch(
                    filter(utils::callback_filter(DROPS_HISTORY_CALLBACK))
                        .endpoint(on_drops_history_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(PROFILE_CALLBACK_PREFIX))
                        .endpoint(on_profile_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(
                        SUPPORT_SELECTED_CALLBACK_PREFIX,
                    ))
                    .endpoint(on_support_selected_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(UNIT_FEEDBACK_CALLBACK_PREFIX))
                        .endpoint(on_unit_report_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(
                        UNIT_ACCEPT_FEEDBACK_CALLBACK_PREFIX,
                    ))
                    .endpoint(on_unit_accept_report_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(TOP_CALLBACK_PREFIX))
                        .endpoint(on_top_callback),
                ),
        )
}
