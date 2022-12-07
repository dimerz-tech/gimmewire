use crate::wireguard::Peer;
use crate::{mongo::Mongo, wireguard};
use configparser::ini::Ini;
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
    #[command(description = "📝 Register, if you are new user.")]
    Register,
    #[command(description = "🚀 Get WireGuard config.")]
    GetConfig,
    #[command(description = "📕 Help")]
    Help,
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
pub async fn admin_handle(
    bot: Bot,
    message: Message,
    cmd: AdminCommands,
    chats: Arc<Mutex<HashMap<UserId, ChatId>>>,
    mongo: Mongo,
    config: Arc<Mutex<Ini>>
) -> Result<(), teloxide::RequestError> {
    let admin_chat_id = config.lock().await.getint("Bot", "AdminId").expect("Cannot find admin chat id").unwrap();
    if message.chat.id != ChatId(admin_chat_id) {
        return Ok(());
    }
    let args: Vec<&str> = message.text().unwrap().split(" ").collect();
    if args.len() != 3 {
        bot.send_message(ChatId(admin_chat_id), "Wrong format")
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
                bot.send_message(ChatId(admin_chat_id), "Cannot find peer")
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
    config: Arc<Mutex<Ini>>
) -> Result<(), teloxide::RequestError> {
    let username = message.chat.username().unwrap_or("None").to_string();
    let user_id = message.from().unwrap().id;
    let admin_chat_id = config.lock().await.getint("Bot", "AdminId").expect("Cannot find admin chat id").unwrap();
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
                bot.send_message(ChatId(admin_chat_id), msg).await?;
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
                            admin_chat_id
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
                            admin_chat_id
                        )
                        .await;
                        return Ok(());
                    }
                    Ok(_) => (),
                }
                // If everything is ok => generate and send config
                if let Ok(config_path) = wireguard::gen_conf(&peer, config) {
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
                                admin_chat_id
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
                                admin_chat_id
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
                        admin_chat_id
                    )
                    .await;
                }
            } else {
                bot.send_message(message.chat.id, "Register first").await?;
            }
        }
        UserCommands::Help => {
            bot.send_message(
                message.chat.id,
                "Hello!😉 Quick start:
0. 📱 Install WireGuard client from App Store.
1. 📝 Register
2. 🚀 Get config
3. 🔥 Open config with WireGuard client
             ",
            )
            .await?;
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
    admin_chat_id: i64
) {
    if let Some(msg) = user_msg {
        match bot.send_message(message.chat.id, msg).await {
            Err(why) => log::error!("{}", why),
            Ok(_) => (),
        }
    }
    if let Some(msg) = admin_msg {
        match bot.send_message(ChatId(admin_chat_id), msg).await {
            Err(why) => log::error!("{}", why),
            Ok(_) => (),
        }
    }
    if let Some(error) = err {
        log::error!("{}", error);
    }
}
