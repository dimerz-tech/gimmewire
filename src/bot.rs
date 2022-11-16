use crate::wireguard::Peer;
use crate::{mongo::Mongo, wireguard};
use mongodb::bson::DateTime;
use std::collections::HashMap;
use teloxide::{prelude::*, types::InputFile, utils::command::BotCommands};

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

pub async fn admin_handle(
    bot: Bot,
    message: Message,
    cmd: AdminCommands,
    chats: HashMap<UserId, ChatId>,
) -> Result<(), teloxide::RequestError> {
    match cmd {
        AdminCommands::Approve => println!(
            "Registered from message{}, admin is {}",
            message.text().unwrap(),
            message.from().unwrap().id
        ),
        AdminCommands::Reject => println!("Rejected from message{}", message.text().unwrap()),
    }
    Ok(())
}

pub async fn user_handle(
    bot: Bot,
    message: Message,
    mongo: Mongo,
    chats: HashMap<ChatId, String>,
) -> Result<(), teloxide::RequestError> {
    let text = match message.text() {
        Some(text) => text,
        None => {
            return Ok(());
        }
    };
    let mut response = String::new();
    if let Ok(command) = UserCommands::parse(text, "gimmewirebot") {
        response = match command {
            UserCommands::Register => {
                let username = message.chat.username().unwrap().to_string();
                if mongo.find_by_name(&username).await.is_some() {
                    "This account is already registered".to_string()
                } else {
                    let id = mongo.count().await + 1;
                    mongo
                        .add(Peer {
                            id: id,
                            username: username,
                            private_key: None,
                            public_key: None,
                            date: DateTime::now(),
                        })
                        .await;
                    "Registered. Now you can get yor config file".to_string()
                }
            }
            UserCommands::Count => {
                let count = mongo.count().await;
                format!("Total: {}", count)
            }
            UserCommands::GetConfig => {
                let username = message.chat.username().unwrap().to_string();
                if let Some(mut peer) = mongo.find_by_name(&username).await {
                    wireguard::add_peer(&mut peer, mongo).await;
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
    };

    bot.send_message(message.chat.id, response).await.unwrap();

    Ok(())
}
