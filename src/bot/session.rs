use crate::{bot::support::SupportCategory, oknoid::DropId};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::Dialogue};

#[derive(Default, Clone)]
pub enum SessionState {
    #[default]
    None,
    WaitBioMessage,
    WaitSupportMessage {
        category: SupportCategory,
    },
    WaitUnitReport {
        drop_id: DropId,
    },
    WaitIndexUrl,
}

pub type Session = Dialogue<SessionState, InMemStorage<SessionState>>;
