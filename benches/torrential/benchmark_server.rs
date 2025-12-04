use std::{env::set_current_dir, sync::Arc};

use axum::{Router, routing::{get, post}};
use dashmap::DashMap;
use log::info;
use tokio::sync::OnceCell;
use torrential::{handlers, set_token, state::AppState, void::{VoidBackendFactory, VoidContextProvider, VoidLibraryProvider}};

pub const TOTAL_BYTES: usize = 312 * 1024 * 1024;
pub const CHUNK_SIZE: usize = 4 * 1024 * 1024;

pub async fn start() {
    if let Ok(working_directory) = std::env::var("WORKING_DIRECTORY") {
        set_current_dir(working_directory).expect("failed to change working directory");
    }

    let shared_state = Arc::new(AppState {
        token: OnceCell::new(),
        context_cache: DashMap::new(),

        metadata_provider: Arc::new(VoidContextProvider::new(TOTAL_BYTES, CHUNK_SIZE, TOTAL_BYTES as u64)),
        backend_factory: Arc::new(VoidBackendFactory::new(1, TOTAL_BYTES as u64)),
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
