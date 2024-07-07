use crate::{PoolClient, PoolClients, PeerMap};

use futures::{FutureExt, StreamExt};
use serde::Deserialize;
use serde_json::from_str;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

pub async fn pool_client_connection (
    ws: WebSocket, 
    id: String, 
    pool_clients: PoolClients, 
    mut pool_client: PoolClient,
    peer_map: PeerMap
) {
    let (pool_client_ws_sender, mut pool_client_ws_rcv) = ws.split();
    let (pool_client_sender, pool_client_rcv) = mpsc::unbounded_channel();

    let pool_client_rcv = UnboundedReceiverStream::new(pool_client_rcv);
    
    tokio::task::spawn(
        pool_client_rcv.forward(pool_client_ws_sender).map(|result| {
            if let Err(e) = result {
                eprintln!("error sending wesocket message: {}", e);
            }
        })
    );

    pool_client.sender = Some(pool_client_sender);
    pool_clients.write().await.insert(id.clone(), pool_client);

    println!("{} connected", id);

    while let Some(result) = pool_client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprint!("error receiving WS message for id: {}: {}",id.clone(), e);
                break;
            }
        };
        pool_client_msg(&id, msg, &pool_clients, &peer_map).await;
    }

    pool_clients.write().await.remove(&id);
    println!("{} disconnected", id);
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Ping,
    Offer(String),
}

async fn pool_client_msg (
    id: &str, 
    msg: Message, 
    pool_clients: &PoolClients,
    peer_map: &PeerMap
) {
    println!("received message from {}: {:?}", id, msg);
    let message = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return,
    };

    match from_str::<ClientMessage>(message) {
        Ok(ClientMessage::Ping) => {
            println!("Ping received from {}", id);
        },
        Ok(ClientMessage::Offer(offer_data)) => {
            println!("Offer received from {}, offer_data:{}", id, offer_data);

            let mut locked = pool_clients.write().await;
            if let Some(pool_client) = locked.get_mut(id) {
                // storing the offer in the pool client object
                pool_client.offer = Some(offer_data.clone());

                // send the offer to the other client peer
                let peer_client_id = match peer_map.read().await.get(id) {
                    Some(peer_client_id_value) => peer_client_id_value.to_string(),
                    None => {
                        println!("No peer client found for id: {}", id);
                        "".to_string()
                    }
                };

                if !peer_client_id.is_empty() {
                    let peer_client_lock = pool_clients.read().await;
                    let peer_client = peer_client_lock.get(&peer_client_id);

                    if let Some(peer_client_value) = peer_client {
                        send_to_pool_client(peer_client_value, offer_data).await;
                    } else {
                        println!("No peer client found for id: {}", peer_client_id);
                    }

                } else {
                    println!("No peer client found for id: {}", id);
                }
            } else {
                println!("Client with id {} not found in pool clients", id);
            }
        },
        Err(e) => {
            eprintln!("error while parsing message to client message: {}", e);
        }
    }
}

// this would be used for offer / ice candidate.
async fn send_to_pool_client(pool_client: &PoolClient, message: String) {
    if let Some(sender) = &pool_client.sender {
        let _ = sender.send(Ok(Message::text(message)));
    }
}

// need to use it, for now, not used anywhere.
// Think about removing these from active pool
// also, where to tell them to exchange information to start connecting with each other.
pub async fn create_peer_mapping(
    pool_client1_id: &String, 
    pool_client2_id: &String, 
    peer_map: &PeerMap,
    pool_clients: &PoolClients
) -> bool {
    // read lock releases after this block.
    {
        // todo - either keep peer map and below implementation as it is.
        // or we can use the .paired field of pool_client object
        println!("waiting for lock on peer map");
        let mapping = peer_map.read().await;
        println!("peer map lock acquired");
        if mapping.get(pool_client1_id).is_some() {
            println!("Mapping already exists for. id1: {} and id2: {}",
                pool_client1_id,
                pool_client2_id
            );
            return false;
        }
    }

    // write lock releases after this block.
    {
        println!("waiting for lock on peer map1");
        let mut write_mapping = peer_map.write().await;
        println!("peer map lock acquired1");
        // either we'll use peer_mapping or the .paired field.
        // to be decided later.
        println!("writing to peer map");
        write_mapping.insert(pool_client1_id.to_string(), pool_client2_id.to_string());
        write_mapping.insert(pool_client2_id.to_string(), pool_client1_id.to_string());
        println!("done wrting to peer map");

        println!("waiting for lock on pool clients");
        let mut read_mapping = pool_clients.write().await;
        println!("pool clients lock acquired");

        if let Some(pool_client1) = read_mapping.get_mut(pool_client1_id) {
            pool_client1.paired = true;
        }
        if let Some(pool_client2) = read_mapping.get_mut(pool_client2_id) {
            pool_client2.paired = true;
        }

        println!("Mapping {} and {} as peers.", 
            pool_client1_id, 
            pool_client2_id
        );
    }

    true
}