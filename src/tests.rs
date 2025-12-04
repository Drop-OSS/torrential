use std::{env::set_current_dir, sync::Arc, time::Duration};

use axum::{Router, routing::{get, post}};
use dashmap::DashMap;
use log::info;
use reqwest::Client;
use serde::Serialize;
use tokio::{runtime::Runtime, sync::OnceCell, time::sleep};

use crate::{handlers, set_token, state::AppState, void::{VoidBackendFactory, VoidContextProvider, VoidLibraryProvider}};

const SERVER_URL: &str = "http://127.0.0.1:5000";
const BENCHMARK_CHUNK_URL: &str = "/api/v1/depot/test-game/v1/chunk_0";
const BENCHMARK_TOKEN: &str = "test-token-for-benchmark";

const TOTAL_BYTES: usize = 312 * 1024 * 1024;
const CHUNK_SIZE: usize = 4 * 1024 * 1024;

#[derive(Serialize)]
pub struct TokenPayload {
    token: String,
}

#[tokio::test]
async fn full() {
    std::thread::spawn(|| {
        let server_rt = Runtime::new().unwrap();
        server_rt.block_on(async {
            start().await;
        })
    });

    sleep(Duration::from_secs(1)).await;

    let client = Client::new();
    println!("Sending token to client");

    client
        .post(format!("{}/token", SERVER_URL))
        .json(&TokenPayload {
            token: String::from(BENCHMARK_TOKEN),
        })
        .send()
        .await
        .unwrap();

    dbg!("Creating group");

    let url = format!("{}{}", SERVER_URL, BENCHMARK_CHUNK_URL);
    dbg!("Sending to url");
    let resp = client.get(url).send().await.unwrap().bytes().await.unwrap();
    assert_eq!(resp.len(), CHUNK_SIZE);
}

pub async fn start() {
    if let Ok(working_directory) = std::env::var("WORKING_DIRECTORY") {
        set_current_dir(working_directory).expect("failed to change working directory");
    }

    let shared_state = Arc::new(AppState {
        token: OnceCell::new(),
        context_cache: DashMap::new(),

        metadata_provider: Arc::new(VoidContextProvider::new(
            TOTAL_BYTES,
            CHUNK_SIZE,
            312 * 1024 * 1024,
        )),
        backend_factory: Arc::new(VoidBackendFactory::new(1, 312 * 1024 * 1024)),
        library_provider: Arc::new(VoidLibraryProvider),
    });

    let app = setup_app(shared_state);

    serve(app).await.unwrap();
}

fn setup_app(shared_state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/v1/depot/{game_id}/{version_name}/{chunk_id}",
            get(handlers::serve_file),
        )
        .route("/token", post(set_token))
        .route("/healthcheck", get(handlers::healthcheck))
        .with_state(shared_state)
}

async fn serve(app: Router) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    info!("started depot server");
    axum::serve(listener, app).await
}
