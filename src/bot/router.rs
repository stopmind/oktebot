use crate::{bot::BotContext, oknoid::OknoId};
use either::Either;
use log::{error, warn};
use std::{collections::HashMap, sync::Arc};
use teloxide::{
    requests::Requester,
    types::{
        CallbackQueryId, Chat, ChatKind, MaybeInaccessibleMessage, Message, MessageKind, Update,
        UpdateKind, User, UserId,
    },
    Bot,
};
use teloxide::types::InaccessibleMessage;

pub trait HandlerArgs {
    fn parse() -> Self;
}

impl HandlerArgs for () {
    fn parse() -> Self {}
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub enum PrivilegeLevel {
    #[default]
    Normal,
    Admin,
    SuperAdmin,
}

#[derive(Default, Copy, Clone)]
pub struct HandlerOptions {
    pub only_private: bool,
    pub check_blacklist: bool,
    pub required_privilege: PrivilegeLevel,
}

impl HandlerOptions {
    pub fn only_private(mut self, val: bool) -> Self {
        self.only_private = val;
        self
    }
    pub fn check_blacklist(mut self, val: bool) -> Self {
        self.check_blacklist = val;
        self
    }
    pub fn required_privilege(mut self, val: PrivilegeLevel) -> Self {
        self.required_privilege = val;
        self
    }

    pub async fn check(
        &self,
        bot: &Bot,
        db: &OknoId,
        chat: &Chat,
        user_id: UserId,
    ) -> anyhow::Result<bool> {
        if self.only_private && !matches!(&chat.kind, ChatKind::Private(..)) {
            bot.send_message(
                chat.id,
                "Действие может быть выполнено только в личных сообщениях.",
            )
            .await?;
            return Ok(false);
        }

        if self.check_blacklist && db.is_user_banned(user_id).await? {
            bot.send_message(chat.id, "Вы забанены.").await?;
            return Ok(false);
        }

        if self.required_privilege == PrivilegeLevel::SuperAdmin && !db.is_super_admin(user_id)
            || self.required_privilege == PrivilegeLevel::Admin
                && !db.check_user_privileges(user_id).await?
        {
            bot.send_message(chat.id, "У вас недостаточно прав").await?;
            return Ok(false);
        }

        Ok(true)
    }
}

struct UniversalHandler(Box<dyn Fn(Arc<BotContext>, UniversalInfo) + Send + Sync>);

impl UniversalHandler {
    fn new<Func, Args, Fut>(f: Func, opts: HandlerOptions) -> Self
    where
        Func: Fn(&BotContext, &UniversalInfo, Args) -> Fut + Send + Sync + 'static,
        Args: HandlerArgs,
        Fut: Future<Output = anyhow::Result<()>> + Send + Sync + 'static,
    {
        todo!()
    }

    fn handle(&self, ctx: Arc<BotContext>, info: UniversalInfo) {
        (self.0)(ctx, info);
    }
}

pub struct UniversalInfo {}

struct CommandHandler(Box<dyn Fn(Arc<BotContext>, Message) + Send + Sync>);

impl CommandHandler {
    fn new<Func, Args, Fut>(f: Func, opts: HandlerOptions) -> Self
    where
        Func: Fn(&BotContext, &Message, Args) -> Fut,
        Args: HandlerArgs,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let handler = move |ctx, info| {
            let f_future = f(Arc::as_ref(&ctx), &info, Args::parse());

            tokio::spawn(async move {
                match opts
                    .check(&ctx.bot, &ctx.db, &info.chat, info.from.id)
                    .await
                {
                    Ok(proceed) => {
                        if !proceed {
                            return;
                        }
                        if let Err(err) = f_future.await {
                            error!("Handler error: {err}")
                        }
                    }
                    Err(err) => error!("Options checking error: {err}"),
                }
            });
        };

        Self(Box::new(handler))
    }

    fn handle(&self, ctx: Arc<BotContext>, info: Message) {
        (self.0)(ctx, info);
    }
}

struct CallbackHandler();

pub struct CallbackInfo {
    pub id: CallbackQueryId,
    pub data: String,
    pub message: MaybeInaccessibleMessage,
    pub from: User,
}

impl CallbackHandler {
    fn new<Func, Args, Fut>(f: Func, opts: HandlerOptions) -> Self
    where
        Func: Fn(&BotContext, &CallbackInfo, Args) -> Fut,
        Args: HandlerArgs,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        todo!()
    }

    fn handle(&self, ctx: Arc<BotContext>, callback: CallbackInfo) {
        todo!()
    }
}

#[derive(Default)]
pub struct Router {
    universal_handlers: Vec<UniversalHandler>,
    commands_actions: HashMap<String, Either<CommandHandler, usize>>,
    callbacks_actions: HashMap<String, Either<CallbackHandler, usize>>,
}

impl Router {
    pub fn universal<Func, Args, Fut>(
        &mut self,
        func: Func,
        command: impl Into<String>,
        callback: impl Into<String>,
        opts: HandlerOptions,
    ) -> &mut Self
    where
        Func: Fn(&BotContext, &UniversalInfo, Args) -> Fut + Send + Sync + 'static,
        Args: HandlerArgs,
        Fut: Future<Output = anyhow::Result<()>> + Send + Sync + 'static,
    {
        let handler = UniversalHandler::new(func, opts);
        let command = command.into();
        let callback = callback.into();

        let idx = self.universal_handlers.len();
        self.universal_handlers.push(handler);

        self.commands_actions.insert(command, Either::Right(idx));
        self.callbacks_actions.insert(callback, Either::Right(idx));

        self
    }

    pub fn callback<Func, Args, Fut>(
        &mut self,
        func: Func,
        callback: impl Into<String>,
        opts: HandlerOptions,
    ) -> &mut Self
    where
        Func: Fn(&BotContext, &CallbackInfo, Args) -> Fut,
        Args: HandlerArgs,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let handler = CallbackHandler::new(func, opts);
        let callback = callback.into();

        self.callbacks_actions
            .insert(callback, Either::Left(handler));
        self
    }

    pub fn command<Func, Args, Fut>(
        &mut self,
        func: Func,
        command: impl Into<String>,
        opts: HandlerOptions,
    ) -> &mut Self
    where
        Func: Fn(&BotContext, &Message, Args) -> Fut,
        Args: HandlerArgs,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let handler = CommandHandler::new(func, opts);
        let command = command.into();

        self.commands_actions.insert(command, Either::Left(handler));
        self
    }

    fn handle_update(&self, ctx: &Arc<BotContext>, update: Update) {
        match update.kind {
            UpdateKind::Message(message) => {
                let Some(text) = message.text() else {
                    return;
                };

                if !text.starts_with('/') {
                    return;
                }

                let mut command = match text.find(' ') {
                    None => text,
                    Some(i) => &text[..i],
                };

                if let Some((command_name, username)) = command.split_once('@') {
                    if username != ctx.me.username.as_ref().unwrap() {
                        return;
                    }

                    command = command_name;
                }

                let Some(action) = self.commands_actions.get(command) else {
                    return;
                };

                match action {
                    Either::Left(handler) => {
                        handler.handle(ctx.clone(), message);
                    }
                    Either::Right(idx) => {
                        self.universal_handlers[*idx].handle(ctx.clone(), UniversalInfo {});
                    }
                }
            }
            UpdateKind::CallbackQuery(callback) => {
                let Some(callback_data) = callback.data else {
                    return;
                };

                let Some(message) = callback.message else {
                    return;
                };

                let name = match callback_data.find('-') {
                    None => callback_data.as_str(),
                    Some(i) => &callback_data[..i],
                };

                let Some(action) = self.callbacks_actions.get(name) else {
                    warn!("Unknown callback: {}", name);
                    return;
                };

                match action {
                    Either::Left(handler) => {
                        handler.handle(ctx.clone(), CallbackInfo {
                            id: callback.id,
                            data: "".to_string(),
                            message,
                            from: callback.from,
                        });
                    }
                    Either::Right(idx) => {
                        self.universal_handlers[*idx].handle(ctx.clone(), UniversalInfo {
                            callback_id: Some(callback.id),
                            from: callback.from,
                            message,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}
