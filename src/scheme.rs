use teloxide::dispatching::{dialogue, UpdateHandler};
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dptree::case;
use teloxide::filter_command;
use teloxide::prelude::*;
use crate::command::Command;
use crate::support::*;
use crate::utils;

pub fn scheme() -> UpdateHandler<anyhow::Error> {
    dialogue::enter::<Update, InMemStorage<SupportState>, SupportState, _>()
        .branch(
            Update::filter_message()
                .branch(
                    filter_command::<Command, _>()
                        .branch(case![Command::Start].endpoint(on_start))
                        .branch(case![Command::Support].endpoint(on_support))
                )
                .branch(
                    case![SupportState::WaitMessage].endpoint(on_support_message)
                )
        )
        .branch(
            Update::filter_callback_query()
                .chain(case![SupportState::WaitMessage])
                .filter(utils::callback_filter(SUPPORT_CANCEL))
                .endpoint(on_support_cancel)
        )
}