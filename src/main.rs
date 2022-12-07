use crate::bot::{admin_handle, user_handle, AdminCommands, UserCommands};
use crate::mongo::Mongo;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{prelude::*, utils::command::BotCommands};
use tokio::sync::Mutex;
use configparser::ini::Ini;
use clap::{Parser, arg, command};
mod bot;
mod mongo;
mod wireguard;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting bot...");
    let args = Args::parse();
    let content = std::fs::read_to_string(&args.config).expect("Cannot read config file");
    let config:Arc<Mutex<Ini>> = Arc::new(Mutex::new(Ini::new()));
    config.blocking_lock().read(content).expect("Cannot parse config");
    let mongo = Mongo::new(&config.blocking_lock().get("db", "url").expect("Cannot find db url"), config.blocking_lock().get("db", "name").expect("Cannot find db name"), config.blocking_lock().get("db", "table").expect("Cannot find db table")).await;
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
        .dependencies(dptree::deps![mongo, chats, config])
        .build()
        .dispatch()
        .await;
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: String
}