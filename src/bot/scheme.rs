use crate::bot::{
    command::Command,
    on_help_callback, on_help_command,
    profile::{
        add_admin, change_rep, del_admin, on_bio, on_bio_cancel, on_bio_message, on_info, on_me,
        on_start, usernames_inspect,
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
                        .branch(case![Command::Rep].endpoint(change_rep)),
                )
                .branch(case![SessionState::WaitSupportMessage].endpoint(on_support_message))
                .branch(case![SessionState::WaitBioMessage].endpoint(on_bio_message)),
        )
        .branch(
            Update::filter_callback_query()
                .branch(
                    filter(utils::callback_filter(CANCEL_CALLBACK))
                        .branch(case![SessionState::WaitSupportMessage].endpoint(on_support_cancel))
                        .branch(case![SessionState::WaitBioMessage].endpoint(on_bio_cancel)),
                )
                .branch(filter(utils::callback_filter(HELP_CALLBACK)).endpoint(on_help_callback)),
        )
}
