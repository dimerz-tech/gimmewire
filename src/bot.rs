use crate::wireguard::Peer;
use crate::{mongo::Mongo, wireguard};
use mongodb::bson::DateTime;
use std::error::Error;
use teloxide::{prelude::*, types::InputFile, utils::command::BotCommands};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "Register, if you are new user.")]
    Register,
    #[command(description = "Get WireGuard config.")]
    GetConfig,
    #[command(description = "Users number")]
    Count,
}

pub async fn handle(
    bot: Bot,
    message: Message,
    mongo: Mongo,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let text = match message.text() {
        Some(text) => text,
        None => {
            return Ok(());
        }
    };
    let mut response = String::new();
    if let Ok(command) = Command::parse(text, "gimmewirebot") {
        response = match command {
            Command::Register => {
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
            Command::Count => {
                let count = mongo.count().await;
                format!("Total: {}", count)
            }
            Command::GetConfig => {
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
