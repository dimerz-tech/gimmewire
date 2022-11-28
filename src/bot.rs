use crate::wireguard::Peer;
use crate::{mongo::Mongo, wireguard};
use mongodb::bson::DateTime;
use simple_error::SimpleError;
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
    #[command(description = "Remove peer")]
    Remove,
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
    match cmd {
        AdminCommands::Approve => {
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
        AdminCommands::Reject => {
            bot.send_message(
                chats.lock().await[&user_id],
                "Sorry, admin's rejected your request",
            )
            .await
            .unwrap();
        }
        AdminCommands::Remove => {
            if let Some(peer) = mongo.find_by_id(user_id.0).await {
                wireguard::remove_peer(&peer, &mongo).await;
                bot.send_message(
                    chats.lock().await[&user_id],
                    "You've been removed from gimmewire",
                )
                .await
                .unwrap();
            } else {
                bot.send_message(ChatId(ADMIN_CHAT_ID), "Cannot find peer")
                    .await
                    .unwrap();
            }
        }
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
    let username = message.chat.username().unwrap_or("None").to_string();
    let response = match cmd {
        UserCommands::Register => {
            if mongo
                .find_by_id(message.from().unwrap().id.0)
                .await
                .is_some()
            {
                "This account is already registered".to_string()
            } else {
                let chat_id = message.chat.id;
                let user_id = message.from().unwrap().id;
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
            let mut response = String::new();
            if let Some(mut peer) = mongo.find_by_name(&username).await {
                // Add peer to wireguard, if err => send message to user and to admin
                match wireguard::add_peer(&mut peer, &mongo).await {
                    Err(why) => {
                        send_and_log_msg(
                            &bot,
                            &message,
                            Some(format!("Cannot add peer {}", peer.username)),
                            Some("Sorry cannot generate config".to_string()),
                            Some(why),
                        )
                        .await;
                        return Ok(());
                    }
                    Ok(_) => (),
                };
                // Update peer in db, if err => send message to user and to admin
                match mongo.update(&peer).await {
                    Err(why) => {
                        wireguard::remove_peer(&peer, &mongo).await; // Something like dummy rollback
                        send_and_log_msg(
                            &bot,
                            &message,
                            Some(format!("Cannot update peer {}", peer.username)),
                            Some("Sorry cannot generate config".to_string()),
                            Some(why),
                        )
                        .await;
                        return Ok(());
                    }
                    Ok(_) => (),
                }
                if let Ok(config_path) = wireguard::gen_conf(&peer) {
                    match bot
                        .send_document(message.chat.id, InputFile::file(config_path))
                        .await
                    {
                        Err(why) => {
                            send_and_log_msg(
                                &bot,
                                &message,
                                Some(format!("Cannot send config to {}", peer.username)),
                                Some("Sorry cannot send config".to_string()),
                                Some(SimpleError::from(why)),
                            )
                            .await;
                        }
                        Ok(_) => (),
                    }
                    match bot
                        .send_message(message.chat.id, "Open it with WireGuard")
                        .await
                    {
                        Err(why) => {
                            send_and_log_msg(
                                &bot,
                                &message,
                                Some(format!("Cannot send success message to {}", peer.username)),
                                None,
                                Some(SimpleError::from(why)),
                            )
                            .await
                        }
                        Ok(_) => (),
                    }
                } else {
                    send_and_log_msg(
                        &bot,
                        &message,
                        Some(format!("Cannot create config for {}", peer.username)),
                        Some("Sorry cannot generate config".to_string()),
                        None,
                    )
                    .await;
                }
            } else {
                response = "Register please".to_string()
            }
            response
        }
    };
    bot.send_message(message.chat.id, response).await.unwrap();

    Ok(())
}

async fn send_and_log_msg(
    bot: &Bot,
    message: &Message,
    admin_msg: Option<String>,
    user_msg: Option<String>,
    err: Option<SimpleError>,
) {
    if let Some(msg) = user_msg {
        match bot.send_message(message.chat.id, msg).await {
            Err(why) => log::error!("{}", why),
            Ok(_) => (),
        }
    }
    if let Some(msg) = admin_msg {
        match bot.send_message(ChatId(ADMIN_CHAT_ID), msg).await {
            Err(why) => log::error!("{}", why),
            Ok(_) => (),
        }
    }
    if let Some(error) = err {
        log::error!("{}", error);
    }
}
