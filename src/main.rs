use std::time::Duration;
use std::{collections::HashMap, sync::Mutex};
use std::convert::Infallible;
use std::sync::Arc;
use std::thread::sleep;
use http::header;
use tokio::sync::{mpsc, RwLock};
use warp::{ws::Message, Filter, Rejection};
use ws::create_peer_mapping;

mod handler;
mod ws;

type Result<T> = std::result::Result<T, Rejection>;
type PoolClients = Arc<RwLock<HashMap<String, PoolClient>>>;
type PeerMap = Arc<RwLock<HashMap<String, String>>>;


#[derive(Debug, Clone)]
pub struct PoolClient {
    pub user_id: String,
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub offer: Option<String>,
}

#[tokio::main]
async fn main() {

    println!("Entered main");

    let pool_clients: PoolClients = Arc::new(RwLock::new(HashMap::new()));
    let peer_map: PeerMap = Arc::new(RwLock::new(HashMap::new()));

    let join_route = warp::path("join")
    .and(warp::post())
    .and_then(handler::join_handler);

    let ws_conn_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with_pool_clients(pool_clients.clone()))
        .and(with_peer_map(peer_map.clone()))
        .and_then(handler::ws_conn_handler);
        
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec![
            header::CONTENT_TYPE
        ])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
        .build();

    let routes = join_route
        .or(ws_conn_route)
        .with(cors);

    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;

    loop {
        find_and_pair_peers(&pool_clients, &peer_map).await;
        sleep(Duration::from_millis(2000));
    }
}

// the problem with this code is that I'm not tracking which clients are already matched
// we don't need to match them again with someone.
async fn find_and_pair_peers(pool_clients: &PoolClients, peer_map: &PeerMap) {
    let pool_lock = pool_clients.read().await;

    let available_clients: Vec<_> = pool_lock.iter().map(|(_, id)| id.clone()).collect();

    for i in (0..available_clients.len()).step_by(2) {
        if i + 1 < available_clients.len() {
            let client1 = &available_clients[i];
            let client2 = &available_clients[i + 1];
            let result = create_peer_mapping(client1, client2, peer_map).await;
            if result {
                println!("Created peer mapping for client1: {} and client2: {}", 
                    client1.user_id, 
                    client2.user_id
                );
            }
        }
    }
}

fn with_peer_map(peer_map: PeerMap) -> impl Filter<Extract = (PeerMap,), Error = Infallible> + Clone {
    warp::any().map(move || peer_map.clone())
}

fn with_pool_clients(pool_clients: PoolClients) -> impl Filter<Extract = (PoolClients,), Error = Infallible> + Clone {
    warp::any().map(move || pool_clients.clone())
}
