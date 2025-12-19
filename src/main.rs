use std::{
    env::{self, set_current_dir},
    sync::Arc,
};

use axum::{
    Router, handler,
    routing::{get, post},
};
use dashmap::DashMap;
use log::info;
use simple_logger::SimpleLogger;
use tokio::{runtime::Handle, sync::OnceCell};
use torrential::{handlers, serve, set_token, state::AppState};
use url::Url;

#[tokio::main]
async fn main() {
    initialise_logger();

    if let Ok(working_directory) = std::env::var("WORKING_DIRECTORY") {
        info!("moving to working directory {}", working_directory);
        set_current_dir(working_directory).expect("failed to change working directory");
    }

    let metrics = Handle::current().metrics();
    info!("using {} threads", metrics.num_workers());

    let shared_state = Arc::new(AppState {
        token: OnceCell::new(),
        context_cache: DashMap::new(),
    });

    let app = setup_app(shared_state);

    serve(app).await.unwrap();
}

fn setup_app(shared_state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/v1/depot/content/{game_id}/{version_name}/{chunk_id}",
            get(serve::serve_file),
        )
        .route("/api/v1/depot/manifest.json", get(handlers::manifest))
        .route("/api/v1/depot/speedtest", get(handlers::speedtest))
        .route("/key", post(set_token))
        .route("/healthcheck", get(handlers::healthcheck))
        .route("/invalidate", post(handlers::invalidate))
        .with_state(shared_state)
}

async fn serve(app: Router) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    info!("started depot server");
    axum::serve(listener, app).await
}

fn initialise_logger() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();
}
