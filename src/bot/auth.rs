use crate::bot::{BotContext, router::CommandInfo};
use log::error;
use teloxide::{payloads::SendMessageSetters, requests::Requester, types::ParseMode};

pub async fn on_auth_command(ctx: &BotContext, info: &CommandInfo, _: ()) -> anyhow::Result<()> {
    match ctx.login.create_token(info.from.id) {
        Ok(token) => {
            let msg = format!("Ваш токен: <code>{token}</code>, его время действия ограничено!");
            ctx.bot
                .send_message(info.chat.id, msg)
                .parse_mode(ParseMode::Html)
                .await?;
            Ok(())
        }
        Err(err) => {
            error!(
                "Failed to get token for user {} ({}): {err}",
                info.from.username.as_ref().unwrap_or(&info.from.first_name),
                info.from.id
            );

            ctx.bot
                .send_message(info.chat.id, "Не удалось получить токен, попробуйте позже.")
                .await?;

            Ok(())
        }
    }
}
