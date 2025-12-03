use std::{
    env::{self, set_current_dir},
    sync::Arc,
};

use axum::{
    Router,
    routing::{get, post},
};
use dashmap::DashMap;
use log::info;
use simple_logger::SimpleLogger;
use tokio::{runtime::Handle, sync::OnceCell};
use torrential::{
    DropBackendFactory, DropContextProvider, DropLibraryProvider, handlers, serve, set_token, state::AppState
};
use url::Url;

#[tokio::main]
async fn main() {
    initialise_logger();

    if let Ok(working_directory) = std::env::var("WORKING_DIRECTORY") {
        set_current_dir(working_directory).expect("failed to change working directory");
    }

    let metrics = Handle::current().metrics();
    info!("using {} threads", metrics.num_workers());

    let remote_url = get_remote_url();

    let shared_state = Arc::new(AppState {
        token: OnceCell::new(),
        context_cache: DashMap::new(),

        metadata_provider: Arc::new(DropContextProvider::new(remote_url.clone())),
        backend_factory: Arc::new(DropBackendFactory),
        library_provider: Arc::new(DropLibraryProvider::new(remote_url)),
    });

    let app = setup_app(shared_state);

    serve(app).await.unwrap();
}

fn setup_app(shared_state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/v1/depot/{game_id}/{version_name}/{*chunk_ids}",
            get(serve::serve_file),
        )
        .route("/token", post(set_token))
        .route("/healthcheck", get(handlers::healthcheck))
        .route("/invalid", post(handlers::invalidate))
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

fn get_remote_url() -> Url {
    let user_provided = env::var("DROP_SERVER_URL");
    let url = Url::parse(
        user_provided
            .as_ref()
            .map_or("http://localhost:3000", |v| v),
    )
    .expect("failed to parse URL");
    info!("using Drop server url {url}");
    url
}
