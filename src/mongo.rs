use crate::wireguard::Peer;
use futures::stream::{StreamExt, TryStreamExt};
use mongodb::{bson::doc, Client};
#[derive(Clone)]
pub struct Mongo {
    client: Client,
}

impl Mongo {
    pub async fn new() -> Self {
        Mongo {
            client: Client::with_uri_str("mongodb://localhost:27017")
                .await
                .unwrap(),
        }
    }

    pub async fn add(&self, peer: &Peer) {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        peers.insert_one(peer, None).await.unwrap();
    }

    pub async fn update(&self, peer: &Peer) {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        peers
            .find_one_and_replace(
                doc! {
                    "user_id": peer.user_id as i64
                },
                peer,
                None,
            )
            .await
            .unwrap();
    }

    pub async fn find_by_id(&self, id: u64) -> Option<Peer> {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        match peers
            .find_one(
                doc! {
                    "user_id": id as i64
                },
                None,
            )
            .await
        {
            Ok(result) => result,
            Err(err) => {
                println!("{}", err);
                None
            }
        }
    }

    pub async fn find_by_name(&self, name: &String) -> Option<Peer> {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        match peers
            .find_one(
                doc! {
                    "username": name
                },
                None,
            )
            .await
        {
            Ok(result) => result,
            Err(err) => {
                println!("{}", err);
                None
            }
        }
    }

    pub async fn delete(&self, peer: Peer) {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        peers
            .delete_one(
                doc! {
                    "user_id": peer.user_id as i64
                },
                None,
            )
            .await
            .unwrap();
    }

    pub async fn count(&self) -> u64 {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        peers.count_documents(None, None).await.unwrap()
    }

    pub async fn get_peers(&self) -> Vec<Peer> {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        peers
            .find(None, None)
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap()
    }
}

#[cfg(test)]
#[tokio::test]
async fn add_peer() {
    let mongo = Mongo::new().await;
    let peer = Peer {
        user_id: 256,
        username: "Name".to_string(),
        public_key: None,
        private_key: None,
        ip: None,
        date: mongodb::bson::DateTime::now(),
    };
    mongo.add(&peer).await;
    let peer = mongo.find_by_id(256).await;
    if let Some(peer) = peer {
        assert!(peer.username == "Name");
        mongo.delete(peer).await;
        assert!(mongo.find_by_id(256).await.is_none())
    } else {
        assert!(false);
    }
}
