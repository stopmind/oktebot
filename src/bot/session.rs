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
}

pub type Session = Dialogue<SessionState, InMemStorage<SessionState>>;
