use crate::{bot::support::SupportCategory, oknoid::DropId};
use std::{collections::BTreeMap, sync::Mutex};
use teloxide::prelude::UserId;

#[derive(Default, Clone, Copy)]
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
        todo!()
    }

    pub fn set(&self, id: UserId, state: SessionState) {
        todo!()
    }

    pub fn update(&self, id: UserId, f: impl FnOnce(&mut SessionState)) {
        todo!()
    }
}
