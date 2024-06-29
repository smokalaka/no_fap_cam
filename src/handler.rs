use crate::{ws, PoolClient, PoolClients, PeerMap, Result};
use serde::{Serialize};
use uuid::Uuid;
use warp::{reply::json, Reply};
use warp::reject::{self};

#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

#[derive(Serialize, Debug)]
pub struct JoinResponse {
    url: String,
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
            offer: None,
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

pub async fn ws_conn_handler (
    ws: warp::ws::Ws, 
    id: String, 
    pool_clients: PoolClients,
    peer_map: PeerMap
) -> Result<impl Reply> {
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
                ws::pool_client_connection(socket, id, pool_clients, c, peer_map)
            )
        ),
        None => Err(warp::reject::not_found()),
    }
}