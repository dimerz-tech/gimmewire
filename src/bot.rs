use crate::wireguard;
use simple_error::SimpleError;
use teloxide::{prelude::*, types::InputFile, utils::command::BotCommands};

pub struct Client {
    pub username: String,
}

pub async fn run() {
    let bot = Bot::from_env();

    teloxide::commands_repl(bot, answer, Command::ty()).await;

    #[derive(BotCommands, Clone)]
    #[command(
        rename_rule = "lowercase",
        description = "These commands are supported:"
    )]
    enum Command {
        #[command(description = "generate and get WireGuard config.")]
        GetConfig,
        #[command(description = "test")]
        Test,
    }

    async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
        match cmd {
            Command::GetConfig => {
                let client = Client {
                    username: msg.chat.username().unwrap_or("user_unknown").to_string(),
                };
                let peer = match wireguard::add_peer(client).await {
                    Err(_) => Err(SimpleError::new("Cannot send message back")),
                    Ok(id) => Ok(id),
                };
                if let Ok(peer) = peer {
                    if let Ok(config_path) = wireguard::gen_conf(&peer) {
                        bot.send_document(msg.chat.id, InputFile::file(config_path))
                            .await
                            .unwrap();
                    } else {
                        bot.send_message(msg.chat.id, "Cannot create config")
                            .await?;
                    }
                } else {
                    bot.send_message(msg.chat.id, "Cannot create peer").await?;
                }
            }
            Command::Test => {
                bot.send_message(msg.chat.id, String::from("Ack"))
                    .await
                    .unwrap();
                println!("{}", msg.chat.username().unwrap());
            }
        };

        Ok(())
    }
}
