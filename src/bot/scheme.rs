use crate::bot::{
    command::Command,
    main_menu_callback, main_menu_command,
    oknounit::{
        drop_command, drops_history_callback, on_unit_report_cancel, on_unit_report_message,
        unit_accept_report_callback, unit_info_callback, unit_info_command, unit_join_callback,
        unit_report_callback, unit_report_command,
    },
    on_help_callback, on_help_command,
    profile::{
        add_admin, change_rep, del_admin, me_callback, on_bio, on_bio_callback, on_bio_cancel,
        on_bio_message, on_info, on_me, on_profile_callback, on_start, top_callback, top_command,
        usernames_inspect,
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
pub const UNIT_REPORT_CALLBACK_PREFIX: &str = "unit-report";
pub const UNIT_ACCEPT_REPORT_CALLBACK_PREFIX: &str = "unit-accept";
pub const DROPS_HISTORY_CALLBACK: &str = "drops-history";
pub const MAIN_MENU_CALLBACK: &str = "main-menu";
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
                        .branch(case![Command::Support].endpoint(on_support))
                        .branch(case![Command::Bio].endpoint(on_bio))
                        .branch(case![Command::Info].endpoint(on_info))
                        .branch(case![Command::Me].endpoint(on_me))
                        .branch(case![Command::AdminAdd].endpoint(add_admin))
                        .branch(case![Command::AdminDel].endpoint(del_admin))
                        .branch(case![Command::Rep].endpoint(change_rep))
                        .branch(case![Command::Top].endpoint(top_command))
                        .branch(case![Command::Unit].endpoint(unit_info_command))
                        .branch(case![Command::UnitReport].endpoint(unit_report_command))
                        .branch(case![Command::Drop].endpoint(drop_command))
                        .branch(case![Command::MainMenu].endpoint(main_menu_command)),
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
                    filter(utils::callback_filter(CANCEL_CALLBACK))
                        .branch(
                            case![SessionState::WaitSupportMessage { category }]
                                .endpoint(on_support_cancel),
                        )
                        .branch(case![SessionState::WaitBioMessage].endpoint(on_bio_cancel))
                        .branch(
                            case![SessionState::WaitUnitReport { drop_id }]
                                .endpoint(on_unit_report_cancel),
                        ),
                )
                .branch(filter(utils::callback_filter(HELP_CALLBACK)).endpoint(on_help_callback))
                .branch(filter(utils::callback_filter(BIO_CALLBACK)).endpoint(on_bio_callback))
                .branch(
                    filter(utils::callback_filter(MAIN_MENU_CALLBACK)).endpoint(main_menu_callback),
                )
                .branch(
                    filter(utils::callback_filter(UNIT_INFO_CALLBACK)).endpoint(unit_info_callback),
                )
                .branch(filter(utils::callback_filter(ME_CALLBACK)).endpoint(me_callback))
                .branch(filter(utils::callback_filter(SUPPORT_CALLBACK)).endpoint(support_callback))
                .branch(
                    filter(utils::callback_filter(UNIT_JOIN_CALLBACK)).endpoint(unit_join_callback),
                )
                .branch(
                    filter(utils::callback_filter(DROPS_HISTORY_CALLBACK))
                        .endpoint(drops_history_callback),
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
                    filter(utils::callback_prefix_filter(UNIT_REPORT_CALLBACK_PREFIX))
                        .endpoint(unit_report_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(
                        UNIT_ACCEPT_REPORT_CALLBACK_PREFIX,
                    ))
                    .endpoint(unit_accept_report_callback),
                )
                .branch(
                    filter(utils::callback_prefix_filter(TOP_CALLBACK_PREFIX))
                        .endpoint(top_callback),
                ),
        )
}
