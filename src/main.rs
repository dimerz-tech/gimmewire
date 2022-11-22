use crate::bot::{admin_handle, user_handle, AdminCommands, UserCommands};
use crate::mongo::Mongo;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{prelude::*, utils::command::BotCommands};
use tokio::sync::Mutex;
mod bot;
mod mongo;
mod wireguard;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting bot...");
    let mongo = Mongo::new().await;
    let bot = Bot::from_env();
    let chats: Arc<Mutex<HashMap<UserId, ChatId>>> = Arc::new(Mutex::new(HashMap::new()));
    bot.set_my_commands(UserCommands::bot_commands())
        .await
        .unwrap();
    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<UserCommands>()
                .endpoint(user_handle),
        )
        .branch(
            dptree::entry()
                .filter_command::<AdminCommands>()
                .endpoint(admin_handle),
        );
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![mongo, chats])
        .build()
        .dispatch()
        .await;
}
