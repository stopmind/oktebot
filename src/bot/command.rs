use teloxide::macros::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "snake_case")]
pub enum Command {
    Start,
    Help,
    Support,
    Bio,
    Me,
    Info(String),
    AdminAdd(String),
    AdminDel(String),
    #[command(parse_with = "split")]
    Rep(String, i64),
}
