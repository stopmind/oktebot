use teloxide::macros::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "snake_case")]
pub enum Command {
    Start,
    Help,
    Support,
    Bio,
    Me,
    Info,
    AdminAdd,
    AdminDel,
    Rep,
    Top,
    Unit,
    UnitReport,
    Drop,
    MainMenu,
}
