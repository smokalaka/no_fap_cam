use crate::{Client, Clients, PoolClient, PoolClients, Result};

use futures::{FutureExt, StreamExt};
use serde::Deserialize;
use serde_json::from_str;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

#[derive(Deserialize, Debug)]
pub struct TopicsRequest {
    topics: Vec<String>,
}

pub async fn pool_client_connection(ws: WebSocket, id: String, pool_clients: PoolClients, 
    mut pool_client: PoolClient) {
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
        pool_client_msg(&id, msg, &pool_clients).await;
    }

    pool_clients.write().await.remove(&id);
    println!("{} disconnected", id);
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Ping,
}

async fn pool_client_msg(id: &str, msg: Message, pool_clients: &PoolClients) {
    println!("received message from {}: {:?}", id, msg);
    let message = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return,
    };

    match from_str::<ClientMessage>(message) {
        Ok(ClientMessage::Ping) => {
            println!("Ping received from {}", id);
        }
        Err(e) => {
            eprintln!("error while parsing message to client message: {}", e);
        }
    }
}

pub async fn client_connection(ws: WebSocket, id: String, clients: Clients, mut client: Client) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    client.sender = Some(client_sender);
    clients.write().await.insert(id.clone(), client);

    println!("{} connected", id);

    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("error receiving ws message for id: {}): {}", id.clone(), e);
                break;
            }
        };
        client_msg(&id, msg, &clients).await;
    }

    clients.write().await.remove(&id);
    println!("{} disconnected", id);
}

async fn client_msg(id: &str, msg: Message, clients: &Clients) {
    println!("received message from {}: {:?}", id, msg);
    let message = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return,
    };

    if message == "ping" || message == "ping\n" {
        return;
    }

    let topics_req: TopicsRequest = match from_str(&message) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error while parsing message to topics request: {}", e);
            return;
        }
    };

    let mut locked = clients.write().await;
    if let Some(v) = locked.get_mut(id) {
        v.topics = topics_req.topics;
    }
}