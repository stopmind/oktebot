use crate::{bot::support::SupportCategory, oknoid::DropId};
use std::{collections::BTreeMap, sync::Mutex};
use teloxide::prelude::UserId;

#[derive(Default, Clone, Copy, Eq, PartialEq)]
pub enum SessionState {
    #[default]
    Empty,
    WaitBioMessage,
    WaitSupportMessage {
        category: SupportCategory,
    },
    WaitUnitReport {
        drop_id: DropId,
    },
}

struct SessionEntry {
    state: SessionState,
}

#[derive(Default)]
pub struct Sessions {
    data: Mutex<BTreeMap<UserId, SessionEntry>>,
}

#[allow(unused)]
impl Sessions {
    pub fn get(&self, id: UserId) -> SessionState {
        self.data.lock().unwrap()
            .get(&id)
            .map(|entry| entry.state)
            .unwrap_or_default()
    }

    pub fn set(&self, id: UserId, state: SessionState) {
        let mut data = self.data.lock().unwrap();
        data.insert(id, SessionEntry { state });
    }

    pub fn update(&self, id: UserId, f: impl FnOnce(&mut SessionState)) {
        let mut data = self.data.lock().unwrap();
        if let Some(entry) = data.get_mut(&id) {
            f(&mut entry.state);
        } else {
            let mut state = SessionState::default();
            f(&mut state);
            if state != SessionState::default() {
                data.insert(id,
                    SessionEntry {
                        state
                    }
                );
            }
        }
    }
}
