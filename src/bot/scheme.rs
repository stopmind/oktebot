use teloxide::dispatching::{dialogue, UpdateHandler};
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dptree::case;
use teloxide::filter_command;
use teloxide::prelude::*;
use crate::bot::command::Command;
use crate::bot::profile::{add_admin, change_rep, del_admin, on_bio, on_bio_cancel, on_bio_message, on_info, on_me, on_start, usernames_inspect};
use crate::bot::session::SessionState;
use crate::bot::support::*;
use crate::bot::utils;

pub const CANCEL_CALLBACK: &str = "cancel";

pub fn scheme() -> UpdateHandler<anyhow::Error> {
    dialogue::enter::<Update, InMemStorage<SessionState>, SessionState, _>()
        .branch(
            Update::filter_message()
                .inspect_async(usernames_inspect)
                .branch(
                    filter_command::<Command, _>()
                        .branch(case![Command::Start].endpoint(on_start))
                        .branch(case![Command::Support].endpoint(on_support))
                        .branch(case![Command::Bio].endpoint(on_bio))
                        .branch(case![Command::Info(mention)].endpoint(on_info))
                        .branch(case![Command::Me].endpoint(on_me))
                        .branch(case![Command::AdminAdd(mention)].endpoint(add_admin))
                        .branch(case![Command::AdminDel(mention)].endpoint(del_admin))
                        .branch(case![Command::Rep(mention, value)].endpoint(change_rep))
                )
                .branch(case![SessionState::WaitSupportMessage].endpoint(on_support_message))
                .branch(case![SessionState::WaitBioMessage].endpoint(on_bio_message))
        )
        .branch(
            Update::filter_callback_query()
                .filter(utils::callback_filter(CANCEL_CALLBACK))
                .branch(case![SessionState::WaitSupportMessage].endpoint(on_support_cancel))
                .branch(case![SessionState::WaitBioMessage].endpoint(on_bio_cancel))
        )
}