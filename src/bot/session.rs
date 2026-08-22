use crate::oknoid::DropId;
use std::sync::Arc;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::Dialogue};

#[derive(Default, Clone)]
pub enum SessionState {
    #[default]
    None,
    WaitBioMessage,
    WaitSupportMessage {
        category: Arc<String>,
    },
    WaitUnitReport {
        drop_id: DropId,
    },
}

pub type Session = Dialogue<SessionState, InMemStorage<SessionState>>;
