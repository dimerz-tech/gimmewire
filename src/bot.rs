use crate::wireguard::Peer;
use crate::{mongo::Mongo, wireguard};
use mongodb::bson::DateTime;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{prelude::*, types::InputFile, utils::command::BotCommands};
use tokio::sync::Mutex;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum UserCommands {
    #[command(description = "Register, if you are new user.")]
    Register,
    #[command(description = "Get WireGuard config.")]
    GetConfig,
    #[command(description = "Users number")]
    Count,
}
#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum AdminCommands {
    #[command(description = "Approve new user.")]
    Approve,
    #[command(description = "Reject new user.")]
    Reject,
}
const ADMIN_CHAT_ID: i64 = 617358980;
pub async fn admin_handle(
    bot: Bot,
    message: Message,
    cmd: AdminCommands,
    chats: Arc<Mutex<HashMap<UserId, ChatId>>>,
    mongo: Mongo,
) -> Result<(), teloxide::RequestError> {
    if message.chat.id != ChatId(ADMIN_CHAT_ID) {
        return Ok(());
    }
    match cmd {
        AdminCommands::Approve => {
            let args: Vec<&str> = message.text().unwrap().split(" ").collect();
            if args.len() != 3 {
                bot.send_message(ChatId(ADMIN_CHAT_ID), "Wrong format")
                    .await
                    .unwrap();
                return Ok(());
            }
            let (username, user_id) = (
                args[1].to_string().strip_prefix("@").unwrap().to_string(),
                UserId(args[2].parse().unwrap()),
            );
            mongo
                .add(&Peer {
                    user_id: user_id.0,
                    username: username,
                    private_key: None,
                    public_key: None,
                    ip: None,
                    date: DateTime::now(),
                })
                .await;
            bot.send_message(
                chats.lock().await[&user_id],
                "Congrats! Generating config.....",
            )
            .await
            .unwrap();
        }
        AdminCommands::Reject => println!("Rejected from message{}", message.text().unwrap()),
    }
    Ok(())
}

pub async fn user_handle(
    bot: Bot,
    message: Message,
    mongo: Mongo,
    cmd: UserCommands,
    chats: Arc<Mutex<HashMap<UserId, ChatId>>>,
) -> Result<(), teloxide::RequestError> {
    let response = match cmd {
        UserCommands::Register => {
            let username = message.chat.username().unwrap().to_string();
            if mongo.find_by_name(&username).await.is_some() {
                "This account is already registered".to_string()
            } else {
                let chat_id = message.chat.id;
                let user_id = message.from().unwrap().id;
                let username = message.chat.username().unwrap();
                let msg = format!("@{} {}", username, user_id);
                chats.lock().await.insert(user_id, chat_id);
                bot.send_message(ChatId(ADMIN_CHAT_ID), msg).await.unwrap();
                "Request is sent to admin".to_string()
            }
        }
        UserCommands::Count => {
            let count = mongo.count().await;
            format!("Total: {}", count)
        }
        UserCommands::GetConfig => {
            let username = message.chat.username().unwrap().to_string();
            if let Some(mut peer) = mongo.find_by_name(&username).await {
                wireguard::add_peer(&mut peer, &mongo).await;
                if let Ok(config_path) = wireguard::gen_conf(&peer) {
                    bot.send_document(message.chat.id, InputFile::file(config_path))
                        .await
                        .unwrap();
                    "Open it with your WireGuard client app".to_string()
                } else {
                    "Cannot create config".to_string()
                }
            } else {
                "Register please".to_string()
            }
        }
    };
    bot.send_message(message.chat.id, response).await.unwrap();

    Ok(())
}
