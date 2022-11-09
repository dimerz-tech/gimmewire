use configparser::ini::Ini;
use mongodb::{
    bson::{doc, DateTime},
    options::ClientOptions,
};
use serde::{Deserialize, Serialize};
use simple_error::{SimpleError, SimpleResult};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::bot;
#[derive(Serialize, Deserialize, Debug)]
pub struct Peer {
    pub id: u64,
    pub username: String,
    pub public_key: String,
    pub private_key: String,
    pub date: DateTime,
}

pub async fn add_peer(client: bot::Client) -> SimpleResult<Peer> {
    let (private_key, public_key) = gen_keys();
    let username = client.username;
    let options = match ClientOptions::parse("mongodb://localhost:27017").await {
        Ok(options) => options,
        Err(why) => {
            println!("Cannot connect to database");
            return Err(SimpleError::from(why));
        }
    };
    let client = mongodb::Client::with_options(options).expect("Cannot create mongo client");
    let peers = client.database("gimmewire").collection::<Peer>("peers");
    let count = match peers.count_documents(None, None).await {
        Ok(count) => count + 1,
        Err(why) => {
            println!("Cannot count documents");
            return Err(SimpleError::from(why));
        }
    };
    let peer = Peer {
        id: count,
        username: username,
        public_key: public_key,
        private_key: private_key,
        date: DateTime::now(),
    };
    match peers.insert_one(&peer, None).await {
        Ok(_) => return Ok(peer),
        Err(why) => {
            println!("Cannot insert peer to db");
            return Err(SimpleError::from(why));
        }
    }
}

pub fn gen_conf(peer: &Peer) -> SimpleResult<String> {
    let mut config = Ini::new_cs();
    config.set("Interface", "PrivateKey", Some(peer.private_key.clone()));
    config.set("Interface", "Address", get_free_ip(peer.id));
    config.set("Interface", "DNS", Some("8.8.8.8".to_string()));
    config.set(
        "Peer",
        "PublicKey",
        Some("kFpzem87OujfORpD9WkVD7vjjESONndZRcT32Dw0xWg=".to_string()),
    );
    config.set("Peer", "Endpoint", Some("194.87.186.2:51820".to_string()));
    config.set("Peer", "AllowedIPs", Some("0.0.0.0/0".to_string()));
    let config_path = format!("/home/amid/{}", peer.username);
    match config.write(&config_path) {
        Err(why) => {
            log::error!("Cannot save a client config: {}", why);
            Err(SimpleError::from(why))
        }
        Ok(_) => Ok(config_path),
    }
}

fn get_free_ip(n: u64) -> Option<String> {
    return Some(format!("10.0.0.2/{}", n + 3));
}

pub fn gen_keys() -> (String, String) {
    let genkey_process = match Command::new("/usr/bin/wg")
        .arg("genkey")
        .stdout(Stdio::piped())
        .spawn()
    {
        Err(why) => panic!("Could not run wg genkey: {}", why),
        Ok(genkey_process) => genkey_process,
    };

    let genkey_output = match genkey_process.wait_with_output() {
        Err(why) => panic!("Could not run wg genkey: {}", why),
        Ok(genkey_output) => genkey_output,
    };

    if !genkey_output.status.success() {
        panic!(
            "wg genkey finished with code {}",
            String::from_utf8(genkey_output.stderr).unwrap()
        );
    }

    let private_key =
        String::from_utf8(genkey_output.stdout).expect("Cannot convert wg genkey to string");

    let mut pubkey_process = match Command::new("/usr/bin/wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Err(why) => panic!("Could not run wg pubkey: {}", why),
        Ok(pubkey_process) => pubkey_process,
    };

    match pubkey_process
        .stdin
        .take()
        .unwrap()
        .write_all(&private_key.as_bytes())
    {
        Err(why) => panic!("Couldn't write to wg pubkey stdin: {}", why),
        Ok(_) => (),
    }

    let pubkey_output = match pubkey_process.wait_with_output() {
        Err(why) => panic!("Could not run wg genkey: {}", why),
        Ok(pubkey_output) => pubkey_output,
    };

    if !pubkey_output.status.success() {
        panic!(
            "wg pubkey finished with code {}",
            String::from_utf8(pubkey_output.stderr).unwrap()
        );
    }
    let public_key =
        String::from_utf8(pubkey_output.stdout).expect("Cannot convert wg pubkey to string");

    (
        private_key.trim().to_string(),
        public_key.trim().to_string(),
    )
}
