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

pub struct CommandParserState<'s> {
    source: &'s str,
}

impl<'s> CommandParserState<'s> {
    pub fn new(source: &'s str) -> Self {
        CommandParserState {
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
            let mut state = $crate::bot::args::CommandParserState::new(args);
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
    fn parse_from_command(text: &str) -> Option<Self>;
    fn parse_from_callback(data: &str) -> Option<Self>;
}

impl HandlerArgs for () {
    fn parse_from_command(_: &str) -> Option<Self> {
        Some(())
    }

    fn parse_from_callback(_: &str) -> Option<Self> {
        Some(())
    }
}

macro_rules! args {
    ($($ty:ident),*) => {
        #[allow(unused)]
        impl<$($ty),*> HandlerArgs for ($($ty,)*)
        where $($ty: FromStr),*
        {
            fn parse_from_command(data: &str) -> Option<Self> {
                let args = data[data.find(' ')?..].trim_start();
                let mut state = CommandParserState::new(args);
                Some((
                    $(state.parse::<$ty>()?,)*
                ))
            }

            fn parse_from_callback(_data: &str) -> Option<Self> {
                todo!()
            }
        }
    };
}

args!(T1);
args!(T1, T2);
args!(T1, T2, T3);
args!(T1, T2, T3, T4);
args!(T1, T2, T3, T4, T5);
