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
    pub paired: bool,
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

    tokio::spawn(async move {
        loop {
            println!("find_and_pair_peers iter enter");
            find_and_pair_peers(&pool_clients, &peer_map).await;
            println!("find_and_pair_peers iter exit");
            sleep(Duration::from_millis(5000));
        }
    });

    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;
}

// the problem with this code is that I'm not tracking which clients are already matched
// we don't need to match them again with someone.
async fn find_and_pair_peers(pool_clients: &PoolClients, peer_map: &PeerMap) {
    println!("trying to find_and_pair_peers");

    // todo, we don't need the paired field, we can check peer_map for is mapped.
    let available_clients: Vec<_> = {
        let pool_lock = pool_clients.read().await;
                                    pool_lock.iter()
                                    .filter_map(|(pool_client_id, pool_client)| {
                                        if pool_client.paired == false {
                                            Some(pool_client_id.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
    };

    println!("available_clients vector: {:?}", available_clients);

    for i in (0..available_clients.len()).step_by(2) {
        if i + 1 < available_clients.len() {
            let client1_id = &available_clients[i];
            let client2_id = &available_clients[i + 1];
            println!("create_peer_mapping starting");
            let result = create_peer_mapping(client1_id, client2_id, peer_map, pool_clients).await;
            println!("create_peer_mapping ending");
            if result {
                println!("Created peer mapping for client1: {} and client2: {}", 
                    client1_id, 
                    client2_id
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
