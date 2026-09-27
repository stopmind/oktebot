use crate::{
    bot::{args::HandlerArgs, BotContext},
    oknoid::OknoId,
};
use anyhow::bail;
use futures::{future::BoxFuture, FutureExt, StreamExt};
use log::{error, warn};
use std::{collections::HashMap, sync::Arc, time::Duration};
use teloxide::{
    requests::Requester,
    types::{
        CallbackQuery, Chat, ChatKind, MaybeInaccessibleMessage, MediaKind, Message, MessageCommon,
        MessageId, MessageKind, Update, UpdateKind, User, UserId,
    },
    update_listeners,
    update_listeners::AsUpdateStream,
    Bot,
};
use crate::bot::args::{CallbackParsingState, CommandParsingState};
use crate::bot::invalid_usage_message;

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
    pub remove_old_message: bool,
}

impl HandlerOptions {
    pub fn empty() -> Self {
        Self::default()
    }

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

    pub fn remove_old_message(mut self, val: bool) -> Self {
        self.remove_old_message = val;
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

struct CommandHandler {
    opts: HandlerOptions,
    func: Box<
        dyn for<'fut> Fn(&'fut BotContext, &'fut CommandInfo) -> BoxFuture<'fut, anyhow::Result<()>>
            + Send
            + Sync,
    >,
}

pub struct CommandInfo {
    pub message_id: MessageId,
    pub from: User,
    pub chat: Chat,
    pub text: String,
}

impl CommandHandler {
    fn new<Func, Args>(f: Func, opts: HandlerOptions) -> Self
    where
        Func: for<'a> Fn(&'a BotContext, &'a CommandInfo, Args) -> BoxFuture<'a, anyhow::Result<()>>
            + Send
            + Sync
            + 'static,
        Args: HandlerArgs,
    {
        Self {
            opts,
            func: Box::new(move |ctx, info| {
                let mut state = CommandParsingState::new(info.text.as_str());
                if let Some(args) = HandlerArgs::parse_from_state(&mut state) {
                    f(ctx, info, args)
                } else {
                    invalid_usage_message(&ctx.bot, info.chat.id)
                        .map(|r| r.map_err(anyhow::Error::from))
                        .boxed()
                }
            }),
        }
    }
}

struct CallbackHandler {
    opts: HandlerOptions,
    func: Box<
        dyn for<'fut> Fn(
                &'fut BotContext,
                &'fut CallbackInfo,
            ) -> Option<BoxFuture<'fut, anyhow::Result<()>>>
            + Send
            + Sync,
    >,
}

pub struct CallbackInfo {
    pub data: String,
    pub message: MaybeInaccessibleMessage,
    pub from: User,
}

impl CallbackHandler {
    fn new<Func, Args>(f: Func, opts: HandlerOptions) -> Self
    where
        Func: for<'a> Fn(&'a BotContext, &'a CallbackInfo, Args) -> BoxFuture<'a, anyhow::Result<()>>
            + Send
            + Sync
            + 'static,
        Args: HandlerArgs,
    {
        Self {
            opts,
            func: Box::new(move |ctx, info| {
                let mut state = CallbackParsingState::new(info.data.as_str());
                HandlerArgs::parse_from_state(&mut state)
                    .map(|args| f(ctx, info, args))
            }),
        }
    }
}


#[derive(Default)]
pub struct Router {
    commands_actions: HashMap<String, CommandHandler>,
    callbacks_actions: HashMap<String, CallbackHandler>,
    state_handler: Option<Box<dyn for<'a> Fn(&'a BotContext, &'a Message) -> Option<BoxFuture<'a, anyhow::Result<()>>> + Send + Sync>>
}

impl Router {
    pub fn callback<Func, Args>(
        &mut self,
        func: Func,
        callback: impl Into<String>,
        opts: HandlerOptions,
    ) -> &mut Self
    where
        Func: for<'a> Fn(&'a BotContext, &'a CallbackInfo, Args) -> BoxFuture<'a, anyhow::Result<()>>
            + Send
            + Sync
            + 'static,
        Args: HandlerArgs,
    {
        let handler = CallbackHandler::new(func, opts);
        let callback = callback.into();

        self.callbacks_actions.insert(callback, handler);
        self
    }

    pub fn command<Func, Args>(
        &mut self,
        func: Func,
        command: impl Into<String>,
        opts: HandlerOptions,
    ) -> &mut Self
    where
        Func: for<'a> Fn(&'a BotContext, &'a CommandInfo, Args) -> BoxFuture<'a, anyhow::Result<()>>
            + Send
            + Sync
            + 'static,
        Args: HandlerArgs,
    {
        let handler = CommandHandler::new(func, opts);
        let command = command.into();

        self.commands_actions.insert(command, handler);
        self
    }

    pub fn state(
        &mut self,
        f: impl for<'a> Fn(&'a BotContext, &'a Message) -> Option<BoxFuture<'a, anyhow::Result<()>>> + Send + Sync + 'static,
    ) -> &mut Self {
        self.state_handler = Some(Box::new(f));
        self
    }
}

impl Router {
    async fn handle_update(&self, ctx: &BotContext, update: Update) -> anyhow::Result<()> {
        match update.kind {
            UpdateKind::Message(message) => self.handle_message(ctx, message).await,
            UpdateKind::CallbackQuery(callback) => self.handle_callback(ctx, callback).await,
            _ => Ok(()),
        }
    }

    async fn handle_message(&self, ctx: &BotContext, message: Message) -> anyhow::Result<()> {
        if let Some(future) = self.state_handler.as_ref()
            .and_then(|handler| handler(ctx, &message))
        {
            return future.await;
        }

        let Message {
            id,
            from: Some(from),
            chat,
            kind:
                MessageKind::Common(MessageCommon {
                    media_kind: MediaKind::Text(text),
                    ..
                }),
            ..
        } = message
        else {
            return Ok(());
        };

        let text = text.text;

        if let Some(command) = text.strip_prefix('/') {
            let mut command = match command.find(' ') {
                None => command,
                Some(pos) => &command[..pos],
            };

            let mut username_specified = false;
            if let Some((command_name, username)) = command.split_once('@') {
                command = command_name;
                username_specified = true;
                if !username.eq_ignore_ascii_case(ctx.me.username()) {
                    return Ok(());
                }
            }

            let Some(handler) = self.commands_actions.get(command) else {
                if username_specified || matches!(&chat.kind, ChatKind::Private(..)) {
                    ctx.bot
                        .send_message(chat.id, "Неизвестная команда.")
                        .await?;
                }
                return Ok(());
            };

            let allowed = handler
                .opts
                .check(&ctx.bot, &ctx.db, &chat, from.id)
                .await?;

            if allowed {
                let info = CommandInfo {
                    message_id: id,
                    from,
                    chat,
                    text,
                };
                (handler.func)(ctx, &info).await?;
            }
        }

        Ok(())
    }

    async fn handle_callback(
        &self,
        ctx: &BotContext,
        callback: CallbackQuery,
    ) -> anyhow::Result<()> {
        let CallbackQuery {
            id,
            data: Some(data),
            from,
            message: Some(message),
            ..
        } = callback
        else {
            return Ok(());
        };

        ctx.bot.answer_callback_query(id).await?;

        let callback_name = match data.find(':') {
            None => data.as_str(),
            Some(pos) => &data[..pos],
        };

        let Some(handler) = self.callbacks_actions.get(callback_name) else {
            bail!("unknown callback: {}", callback_name);
        };

        let allowed = handler
            .opts
            .check(&ctx.bot, &ctx.db, message.chat(), from.id)
            .await?;

        if allowed {
            if handler.opts.remove_old_message
                && let Err(err) = ctx.bot.delete_message(message.chat().id, message.id()).await
            {
                warn!("Failed to delete old message: {}", err);
            }

            let info = CallbackInfo {
                data,
                message,
                from,
            };
            if let Some(future) = (handler.func)(ctx, &info) {
                future.await?;
            }
        }

        Ok(())
    }
}

impl Router {
    pub async fn handle_updates(self: Arc<Self>, ctx: Arc<BotContext>) {
        const RETRY_TIME: Duration = Duration::from_secs(5);

        let mut listener = update_listeners::polling_default(ctx.bot.clone()).await;
        let mut stream = Box::pin(listener.as_stream());

        loop {
            if let Some(res) = stream.next().await {
                match res {
                    Ok(update) => {
                        tokio::spawn({
                            let this = self.clone();
                            let ctx = ctx.clone();

                            async move {
                                if let Err(err) = this.handle_update(&ctx, update).await {
                                    error!("Failed to handle update: {}", err);
                                }
                            }
                        });
                    }
                    Err(err) => {
                        error!(
                            "failed retrieve error: {err}, retry in {}",
                            RETRY_TIME.as_secs()
                        );
                        tokio::time::sleep(RETRY_TIME).await;
                    }
                }
            }
        }
    }
}

#[macro_export]
macro_rules! w {
    ($f:expr) => {
        #[inline]
        |c, i, a| ::futures::FutureExt::boxed(($f)(c, i, a))
    };
}

#[macro_export]
macro_rules! __state_arg {
    ($user:expr, [user]) => {$user};
    ($user:expr, $exp:expr) => {$exp};
}

#[macro_export]
macro_rules! states {
    {$($pattern:pat => $func:ident $(($($arg:tt),+))?),*} => {
        |ctx, message| {
            let __user = message.from.as_ref()?;
            match ctx.sessions.get(__user.id) {
                $($pattern => {::std::option::Option::Some(
                    ::futures::FutureExt::boxed($func(ctx, message, ($($($crate::__state_arg!(__user, $arg),)+)?)))
                )})*
                _ => ::std::option::Option::None
            }
        }
    };
}
