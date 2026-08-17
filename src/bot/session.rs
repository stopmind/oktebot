use teloxide::{dispatching::dialogue::InMemStorage, prelude::Dialogue};

#[derive(Default, Clone)]
pub enum SessionState {
    #[default]
    None,
    WaitBioMessage,
    WaitSupportMessage,
}

pub type Session = Dialogue<SessionState, InMemStorage<SessionState>>;
