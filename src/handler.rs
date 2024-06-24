use crate::{ws, Client, Clients, PoolClient, PoolClients, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp::{http::StatusCode, reply::json, ws::Message, Reply};
use warp::reject::{self};

#[derive(Deserialize, Debug)]
pub struct RegisterRequest {
    user_id: usize,
    topic: String,
}

#[derive(Deserialize)]
pub struct TopicActionRequest {
    topic: String,
    client_id: String,
}


#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

#[derive(Serialize, Debug)]
pub struct JoinResponse {
    url: String,
}

#[derive(Deserialize, Debug)]
pub struct Event {
    topic: String,
    user_id: Option<usize>,
    message: String,
}

pub async fn publish_handler(body: Event, clients: Clients) -> Result<impl Reply> {
    clients
        .read()
        .await
        .iter()
        .filter(|(_, client)| match body.user_id {
            Some(v) => client.user_id == v,
            None => true,
        })
        .filter(|(_, client)| client.topics.contains(&body.topic))
        .for_each(|(_, client)| {
            if let Some(sender) = &client.sender {
                let _ = sender.send(Ok(Message::text(body.message.clone())));
            }
        });

    Ok(StatusCode::OK)
}

pub async fn join_handler() -> Result<impl Reply> {
    let uuid = Uuid::new_v4().as_simple().to_string();

    Ok(json(&JoinResponse {
        url: format!("ws://127.0.0.1:8000/ws/{}", uuid),
    }))
}

async fn add_to_client_pool(id: String, pool_clients: PoolClients) -> bool {
    println!("Adding to client pool. id: {}", id);

    let pool_client = pool_clients.read().await.get(&id).cloned();

    if let Some(_) = pool_client {
        println!("Client already exists in the client pool. id: {}", id);
        return false;
    }

    pool_clients.write().await.insert(
        id.clone(),
        PoolClient {
            user_id: id.clone(),
            sender: None,
        },
    );

    println!("Client added to client pool. id: {}", id);
    println!("Pool size: {}", pool_clients.read().await.len());
    return true;
}

#[derive(Debug)]
struct ClientAlreadyExists {
    reason: String,
}

impl warp::reject::Reject for ClientAlreadyExists {}

pub async fn ws_conn_handler(ws: warp::ws::Ws, id: String, pool_clients: PoolClients) -> Result<impl Reply> {
    println!("WS connection request received {}", id);

    let client_added_result: bool = add_to_client_pool(id.clone(), pool_clients.clone()).await;
    
    if client_added_result == false {
        return Err(reject::custom(ClientAlreadyExists {
            reason: format!("Client already exists in the client pool. id: {}", id),
        }));
    }

    let pool_client = pool_clients.read().await.get(&id).cloned();
    match pool_client {
        Some(c) => Ok(
            ws.on_upgrade(move |socket| 
                ws::pool_client_connection(socket, id, pool_clients, c)
            )
        ),
        None => Err(warp::reject::not_found()),
    }
}

pub async fn register_handler(body: RegisterRequest, clients: Clients) -> Result<impl Reply> {
    let user_id = body.user_id;
    let topic = body.topic; // Capture the entry topic
    let uuid = Uuid::new_v4().as_simple().to_string();

    register_client(uuid.clone(), user_id, topic, clients).await; // Pass the entry topic
    Ok(json(&RegisterResponse {
        url: format!("ws://127.0.0.1:8000/ws/{}", uuid),
    }))
}

async fn register_client(id: String, user_id: usize, topic: String, clients: Clients) {
    clients.write().await.insert(
        id,
        Client {
            user_id,
            topics: vec![topic],
            sender: None,
        },
    );
}

pub async fn unregister_handler(id: String, clients: Clients) -> Result<impl Reply> {
    clients.write().await.remove(&id);
    Ok(StatusCode::OK)
}

pub async fn ws_handler(ws: warp::ws::Ws, id: String, clients: Clients) -> Result<impl Reply> {
    let client = clients.read().await.get(&id).cloned();
    match client {
        Some(c) => Ok(ws.on_upgrade(move |socket| ws::client_connection(socket, id, clients, c))),
        None => Err(warp::reject::not_found()),
    }
}

pub async fn health_handler() -> Result<impl Reply> {
    Ok(StatusCode::OK)
}



pub async fn add_topic(body: TopicActionRequest, clients: Clients) -> Result<impl Reply> {
    let mut clients_write = clients.write().await;
    if let Some(client) = clients_write.get_mut(&body.client_id) {
        client.topics.push(body.topic);
    }
    Ok(warp::reply::with_status("Added topic successfully", StatusCode::OK))
}

pub async fn remove_topic(body: TopicActionRequest, clients: Clients) -> Result<impl Reply> {
    let mut clients_write = clients.write().await;
    if let Some(client) = clients_write.get_mut(&body.client_id) {
        client.topics.retain(|t| t != &body.topic);
    }
    Ok(warp::reply::with_status("Removed topic successfully", StatusCode::OK))
}