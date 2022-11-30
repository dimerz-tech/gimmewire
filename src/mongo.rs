use crate::wireguard::Peer;
use futures::stream::TryStreamExt;
use mongodb::{bson::doc, Client};
use simple_error::{SimpleError, SimpleResult};
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

    pub async fn add(&self, peer: &Peer) -> SimpleResult<()> {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        match peers.insert_one(peer, None).await {
            Err(why) => {
                log::error!("Cannot add peer to db {}", why.to_string());
                Err(SimpleError::from(why))
            }
            Ok(_) => Ok(()),
        }
    }

    pub async fn update(&self, peer: &Peer) -> SimpleResult<()> {
        match self.delete(&peer).await {
            Err(why) => {
                log::error!("Cannot update peer {}", why.to_string());
                return Err(why);
            }
            Ok(_) => match self.add(&peer).await {
                Err(why) => {
                    log::error!("Cannot update peer {}", why.to_string());
                    return Err(why);
                }
                Ok(_) => Ok(()),
            },
        }
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

    pub async fn delete(&self, peer: &Peer) -> SimpleResult<()> {
        let peers = self
            .client
            .database("gimmewire")
            .collection::<Peer>("peers");
        match peers
            .delete_one(
                doc! {
                    "user_id": peer.user_id as i64
                },
                None,
            )
            .await
        {
            Err(why) => {
                log::error!("Cannot delete peer from db {}", why.to_string());
                return Err(SimpleError::from(why));
            }
            Ok(_) => Ok(()),
        }
    }
    #[cfg(test)]
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
async fn test_db() {
    use std::net::Ipv4Addr;

    let mongo = Mongo::new().await;
    let peer1 = Peer {
        user_id: 256,
        username: "User1".to_string(),
        public_key: None,
        private_key: None,
        ip: None,
        date: mongodb::bson::DateTime::now(),
    };
    let peer2 = Peer {
        user_id: 256,
        username: "User2".to_string(),
        public_key: None,
        private_key: None,
        ip: Some(Ipv4Addr::new(234, 32, 32, 234)),
        date: mongodb::bson::DateTime::now(),
    };
    let count = mongo.count().await;
    mongo.add(&peer1).await.unwrap();
    mongo.update(&peer2).await.unwrap();
    let peers = mongo.get_peers().await;
    assert!(peers.len() as u64 == count + 1);
    let peer = mongo.find_by_id(256).await;
    if let Some(peer) = peer {
        assert!(peer.username == "User2");
        mongo.delete(&peer).await.unwrap();
        assert!(mongo.find_by_id(256).await.is_none())
    } else {
        assert!(false);
    }
}
