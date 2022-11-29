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
            .await?;
        return Ok(());
    }
    let (username, user_id) = (
        args[1].to_string().strip_prefix("@").unwrap().to_string(),
        UserId(args[2].parse().unwrap()),
    );
    match cmd {
        AdminCommands::Approve => {
            if mongo
                .add(&Peer {
                    user_id: user_id.0,
                    username: username,
                    private_key: None,
                    public_key: None,
                    ip: None,
                    date: DateTime::now(),
                })
                .await
                .is_ok()
            {
                bot.send_message(
                    chats.lock().await[&user_id],
                    "Congrats! Admin's approved your request, now you can get a config",
                )
                .await?;
            }
        }
        AdminCommands::Reject => {
            bot.send_message(
                chats.lock().await[&user_id],
                "Sorry, admin's rejected your request",
            )
            .await?;
        }
        AdminCommands::Remove => {
            if let Some(peer) = mongo.find_by_id(user_id.0).await {
                wireguard::remove_peer(&peer).await;
                if mongo.delete(&peer).await.is_ok() {
                    bot.send_message(
                        chats.lock().await[&user_id],
                        "You've been removed from gimmewire",
                    )
                    .await?;
                }
            } else {
                bot.send_message(ChatId(ADMIN_CHAT_ID), "Cannot find peer")
                    .await?;
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
    let user_id = message.from().unwrap().id;
    match cmd {
        UserCommands::Register => {
            if mongo
                .find_by_id(message.from().unwrap().id.0)
                .await
                .is_some()
            {
                bot.send_message(message.chat.id, "This account is already registered")
                    .await?;
            } else {
                let chat_id = message.chat.id;
                let msg = format!("@{} {}", username, user_id);
                chats.lock().await.insert(user_id, chat_id);
                bot.send_message(ChatId(ADMIN_CHAT_ID), msg).await?;
                bot.send_message(message.chat.id, "Request is sent to admin")
                    .await?;
            }
        }
        UserCommands::GetConfig => {
            if let Some(mut peer) = mongo.find_by_id(user_id.0).await {
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
                        wireguard::remove_peer(&peer).await; // Something like dummy rollback
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
                // If everything is ok => generate and send config
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
                            wireguard::remove_peer(&peer).await; // Something like dummy rollback
                            return Ok(());
                        }
                        Ok(_) => (),
                    }
                    // If everything is ok => send message to user
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
                bot.send_message(message.chat.id, "Register first").await?;
            }
        }
    };

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
