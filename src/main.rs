use crate::bot::{handle, Command};
use crate::mongo::Mongo;
use teloxide::{prelude::*, utils::command::BotCommands};
mod bot;
mod mongo;
mod wireguard;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting bot...");
    let mongo = Mongo::new().await;
    let bot = Bot::from_env();
    bot.set_my_commands(Command::bot_commands()).await.unwrap();
    let handler = dptree::entry().branch(Update::filter_message().endpoint(handle));
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![mongo])
        .build()
        .dispatch()
        .await;
}
