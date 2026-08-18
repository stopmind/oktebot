use crate::oknoid::OknoId;
use std::str::FromStr;
use teloxide::{prelude::UserId, types::Message};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("invalid mention")]
pub struct InvalidMentionError;

#[derive(Clone)]
pub enum Mention {
    Username(String),
    UserId(UserId),
}

impl Mention {
    pub fn resolve(&self, db: &OknoId) -> Option<UserId> {
        match self {
            Mention::Username(username) => db.resolve_username(username),
            Mention::UserId(id) => Some(*id),
        }
    }
}

impl FromStr for Mention {
    type Err = InvalidMentionError;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        if let Some(username) = val.strip_prefix("@") {
            Ok(Mention::Username(username.to_owned()))
        } else {
            match val.parse() {
                Ok(id) => Ok(Mention::UserId(UserId(id))),
                Err(_) => Err(InvalidMentionError),
            }
        }
    }
}

pub struct ParserState<'s> {
    source: &'s str,
}

impl<'s> ParserState<'s> {
    pub fn new(source: &'s str) -> Self {
        ParserState {
            source: source.trim_start(),
        }
    }

    pub fn parse<T: FromStr>(&mut self) -> Option<T> {
        let current;
        (current, self.source) = self
            .source
            .split_once(char::is_whitespace)
            .unwrap_or((self.source, ""));

        self.source = self.source.trim_start();
        current.parse().ok()
    }

    pub fn is_empty(&self) -> bool {
        self.source.is_empty()
    }
}

#[macro_export]
macro_rules! parser {
    [$($ty:ty),*] => {
        |args: &str| -> Option<_> {
            let mut state = $crate::bot::args::ParserState::new(args);
            let res = ($(state.parse::<$ty>()?),*);
            if state.is_empty() {Some(res)}
            else {None}
        }
    }
}

pub fn get_args(message: &Message) -> &str {
    if let Some(text) = message.text()
        && let Some((_, args)) = text.split_once(char::is_whitespace)
    {
        args.trim_start()
    } else {
        ""
    }
}
