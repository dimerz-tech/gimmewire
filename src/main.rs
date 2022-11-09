mod bot;
mod wireguard;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting bot...");
    bot::run().await;
}
