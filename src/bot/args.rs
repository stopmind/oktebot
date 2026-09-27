use std::str::FromStr;
use teloxide::prelude::UserId;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("invalid mention")]
pub struct InvalidMentionError;

#[derive(Clone)]
pub enum Mention {
    Username(String),
    Firstname(String),
    UserId(UserId),
}

impl FromStr for Mention {
    type Err = InvalidMentionError;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        if let Some(username) = val.strip_prefix("@") {
            Ok(Mention::Username(username.to_owned()))
        } else if let Ok(id) = val.parse() {
            Ok(Mention::UserId(UserId(id)))
        } else {
            Ok(Mention::Firstname(val.to_owned()))
        }
    }
}

pub trait ParsingState {
    fn parse<T: FromStr>(&mut self) -> Option<T>;
}

pub struct CommandParsingState<'s> {
    args: &'s str,
}

impl<'s> CommandParsingState<'s> {
    pub fn new(data: &'s str) -> Self {
        let args = data.find(' ')
            .map(|i| data[i..].trim_start())
            .unwrap_or("");
        CommandParsingState { args }
    }
}

impl ParsingState for CommandParsingState<'_> {
    fn parse<T: FromStr>(&mut self) -> Option<T> {
        let current;
        (current, self.args) = self
            .args
            .split_once(char::is_whitespace)
            .unwrap_or((self.args, ""));

        self.args = self.args.trim_start();
        current.parse().ok()
    }
}

pub struct CallbackParsingState<'s> {
    args: &'s str,
}

impl<'s> CallbackParsingState<'s> {
    pub fn new(data: &'s str) -> Self {
        let args = data.find(':')
            .map(|i| &data[i+1..])
            .unwrap_or("");
        CallbackParsingState { args }
    }
}

impl ParsingState for CallbackParsingState<'_> {
    fn parse<T: FromStr>(&mut self) -> Option<T> {
        let current;
        (current, self.args) = self
            .args
            .split_once(':')
            .unwrap_or((self.args, ""));

        current.parse().ok()
    }
}

#[macro_export]
macro_rules! parser {
    [$($ty:ty),*] => {
        |args: &str| -> Option<_> {
            let mut state = $crate::bot::args:CommandParserStatee::new(args);
            let res = ($(state.parse::<$ty>()?),*);
            if state.is_empty() {Some(res)}
            else {None}
        }
    }
}

pub fn get_args(text: &str) -> &str {
    if let Some((_, args)) = text.split_once(char::is_whitespace) {
        args.trim_start()
    } else {
        ""
    }
}

pub trait HandlerArgs: Sized {
    fn parse_from_state<S: ParsingState>(state: &mut S) -> Option<Self>;
}

impl HandlerArgs for () {
    fn parse_from_state<S: ParsingState>(_: &mut S) -> Option<Self> {
        Some(())
    }
}

macro_rules! args {
    ($($ty:ident),*) => {
        #[allow(unused)]
        impl<$($ty),*> HandlerArgs for ($($ty,)*)
        where $($ty: FromStr),*
        {
            fn parse_from_state<S: ParsingState>(state: &mut S) -> Option<Self> {
                Some(($(state.parse::<$ty>()?,)*))
            }
        }
    };
}

args!(T1);
args!(T1, T2);
args!(T1, T2, T3);
args!(T1, T2, T3, T4);
args!(T1, T2, T3, T4, T5);
