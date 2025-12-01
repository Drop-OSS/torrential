use anyhow::Result;
use dashmap::DashMap;
use droplet_rs::versions::types::{VersionBackend, VersionFile};
use reqwest::header;
use simple_logger::SimpleLogger;
use std::{
    collections::HashMap, env::set_current_dir, path::PathBuf, str::FromStr, sync::Arc,
    time::Instant,
};
use tokio_util::io::ReaderStream;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{AppendHeaders, IntoResponse},
    routing::{get, post},
};
use log::{error, info, warn};
use serde::Deserialize;
use tokio::sync::{OnceCell, Semaphore};

use crate::{
    download::create_download_context,
    remote::{LibraryBackend, fetch_library_sources},
};

mod download;
mod manifest;
mod remote;
mod util;

static GLOBAL_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);

struct DownloadContext<'a> {
    chunk_lookup_table: HashMap<String, (String, usize, usize)>,
    backend: Box<dyn VersionBackend + Send + Sync + 'a>,
    last_access: Instant,
}

#[derive(Debug)]
struct AppInitData {
    token: String,
    libraries: HashMap<String, (PathBuf, LibraryBackend)>,
}

struct AppState<'a> {
    token: OnceCell<AppInitData>,
    context_cache: DashMap<(String, String), DownloadContext<'a>>,
}

async fn serve_file(
    State(state): State<Arc<AppState<'_>>>,
    Path((game_id, version_name, chunk_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let init_data = state.token.get().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let key = (game_id.clone(), version_name.clone());

    let mut context = if let Some(context) = state.context_cache.get_mut(&key) {
        context
    } else {
        let permit = GLOBAL_CONTEXT_SEMAPHORE
            .acquire()
            .await
            .expect("failed to acquire semaphore");

        // Check if it's been done while we've been sitting here
        if let Some(already_done) = state.context_cache.get_mut(&key) {
            already_done
        } else {
            info!("generating context...");
            let context_result =
                create_download_context(init_data, game_id.clone(), version_name.clone()).await;
            info!("cleaned up semaphore");

            let new_context = context_result.inspect_err(|v| warn!("{:?}", v))?;
            state.context_cache.insert(key.clone(), new_context);

            info!("continuing download");

            drop(permit);

            state.context_cache.get_mut(&key).unwrap()
        }
    };

    context.last_access = Instant::now();

    let (relative_filename, start, end) = context
        .chunk_lookup_table
        .get(&chunk_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    let reader = context
        .backend
        .reader(
            &VersionFile {
                relative_filename: relative_filename.to_string(),
                permission: 0,
                size: 0,
            },
            start.try_into().unwrap(),
            end.try_into().unwrap(),
        )
        .await
        .map_err(|v| {
            error!("reader error: {v:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let stream = ReaderStream::new(reader);
    let body = Body::from_stream(stream);

    let headers: AppendHeaders<[(header::HeaderName, String); 2]> = AppendHeaders([
        (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
        (header::CONTENT_LENGTH, (end - start).to_string()),
    ]);

    Ok((headers, body))
}

#[derive(Deserialize)]
struct TokenPayload {
    token: String,
}

async fn set_token(
    State(state): State<Arc<AppState<'_>>>,
    Json(payload): Json<TokenPayload>,
) -> Result<StatusCode, StatusCode> {
    if let Some(existing_data) = state.token.get() {
        if existing_data.token != payload.token {
            panic!("already set up but provided with a different token");
        }
        return Ok(StatusCode::OK);
    }

    let token = payload.token;

    let library_sources = fetch_library_sources(token.clone()).await.map_err(|v| {
        error!("{v:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let valid_library_sources = library_sources
        .into_iter()
        .filter(|v| {
            matches!(
                v.backend,
                remote::LibraryBackend::Filesystem | remote::LibraryBackend::FlatFilesystem
            )
        })
        .map(|v| {
            let path = PathBuf::from_str(
                v.options
                    .as_object()
                    .unwrap()
                    .get("baseDir")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap();

            (v.id, (path, v.backend))
        })
        .collect::<HashMap<String, (PathBuf, LibraryBackend)>>();

    state
        .token
        .set(AppInitData {
            token,
            libraries: valid_library_sources,
        })
        .map_err(|err| {
            error!("failed to set token: {err:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("connected to drop server successfully");

    Ok(StatusCode::OK)
}

#[tokio::main]
async fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    if let Ok(working_directory) = std::env::var("WORKING_DIRECTORY") {
        set_current_dir(working_directory).expect("failed to change working directory");
    }

    let shared_state = Arc::new(AppState {
        token: OnceCell::new(),
        context_cache: DashMap::new(),
    });

    let app = Router::new()
        .route(
            "/api/v1/depot/{game_id}/{version_name}/{chunk_id}",
            get(serve_file),
        )
        .route("/token", post(set_token))
        .with_state(shared_state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    info!("started depot server");
    axum::serve(listener, app).await.unwrap();
}
