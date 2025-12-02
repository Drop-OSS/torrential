use anyhow::Result;
use dashmap::{DashMap, mapref::one::RefMut};
use droplet_rs::versions::types::{MinimumFileObject, VersionBackend, VersionFile};
use reqwest::header;
use simple_logger::SimpleLogger;
use std::{
    collections::HashMap, env::set_current_dir, path::PathBuf, str::FromStr as _, sync::Arc,
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
use log::{error, info};
use serde::Deserialize;
use tokio::sync::{OnceCell, Semaphore, SemaphorePermit};

use crate::{
    download::create_download_context,
    remote::{LibraryBackend, LibrarySource, fetch_library_sources},
};

mod download;
mod manifest;
mod remote;
mod util;

static GLOBAL_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);

struct DownloadContext {
    chunk_lookup_table: HashMap<String, (String, usize, usize)>,
    backend: Box<dyn VersionBackend + Send + Sync + 'static>,
    last_access: Instant,
}

#[derive(Debug)]
struct AppInitData {
    token: String,
    libraries: HashMap<String, (PathBuf, LibraryBackend)>,
}

struct AppState {
    token: OnceCell<AppInitData>,
    context_cache: DashMap<(String, String), DownloadContext>,
}

#[tokio::main]
async fn main() {
    initialise_logger();

    if let Ok(working_directory) = std::env::var("WORKING_DIRECTORY") {
        set_current_dir(working_directory).expect("failed to change working directory");
    }

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
            "/api/v1/depot/{game_id}/{version_name}/{chunk_id}",
            get(serve_file),
        )
        .route("/token", post(set_token))
        .route("/healthcheck", get(healthcheck))
        .with_state(shared_state)
}
async fn serve(app: Router) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    info!("started depot server");
    axum::serve(listener, app).await
}

async fn set_token(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TokenPayload>,
) -> Result<StatusCode, StatusCode> {
    if check_token_exists(&state, &payload) {
        return Ok(StatusCode::OK);
    }

    let token = payload.token;

    let library_sources = fetch_library_sources(&token).await.map_err(|v| {
        error!("{v:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let valid_library_sources = filter_library_sources(library_sources);

    set_generated_token(&state, token, valid_library_sources)?;

    info!("connected to drop server successfully");

    Ok(StatusCode::OK)
}

async fn serve_file(
    State(state): State<Arc<AppState>>,
    Path((game_id, version_name, chunk_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let context_cache = &state.context_cache;

    let mut context = get_or_generate_context(&state, context_cache, game_id, version_name).await?;
    context.last_access = Instant::now();

    let (relative_filename, start, end) = lookup_chunk(&chunk_id, &context)?;
    let reader = get_file_reader(&mut context, relative_filename, start, end).await?;

    let stream = ReaderStream::new(reader);
    let body: Body = Body::from_stream(stream);

    let headers: AppendHeaders<[(header::HeaderName, String); 2]> = AppendHeaders([
        (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
        (header::CONTENT_LENGTH, (end - start).to_string()),
    ]);

    Ok((headers, body))
}

fn initialise_logger() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();
}

async fn acquire_permit<'a>() -> SemaphorePermit<'a> {
    return GLOBAL_CONTEXT_SEMAPHORE
        .acquire()
        .await
        .expect("failed to acquire semaphore")
}
fn lookup_chunk(
    chunk_id: &String,
    context: &RefMut<'_, (String, String), DownloadContext>,
) -> Result<(String, usize, usize), StatusCode> {
    context
        .chunk_lookup_table
        .get(chunk_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)
}
async fn get_file_reader(
    context: &mut RefMut<'_, (String, String), DownloadContext>,
    relative_filename: String,
    start: usize,
    end: usize,
) -> Result<Box<dyn MinimumFileObject>, StatusCode> {
    context
        .backend
        .reader(
            &VersionFile {
                relative_filename: relative_filename.clone(),
                permission: 0,
                size: 0,
            },
            start as u64,
            end as u64,
        )
        .await
        .map_err(|v| {
            error!("reader error: {v:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
async fn get_or_generate_context<'a>(
    state: &Arc<AppState>,
    context_cache: &'a DashMap<(String, String), DownloadContext>,
    game_id: String,
    version_name: String,
) -> Result<RefMut<'a, (String, String), DownloadContext>, StatusCode> {
    let initialisation_data = state.token.get().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let key = (game_id.clone(), version_name.clone());

    if let Some(context) = context_cache.get_mut(&key) {
        Ok(context)
    } else {
        let permit = acquire_permit().await;

        // Check if it's been done while we've been sitting here
        if let Some(already_done) = context_cache.get_mut(&key) {
            Ok(already_done)
        } else {
            info!("generating context...");
            let context_result =
                create_download_context(initialisation_data, game_id.clone(), version_name.clone())
                    .await?;

            state.context_cache.insert(key.clone(), context_result);

            info!("continuing download");

            drop(permit);

            Ok(context_cache.get_mut(&key).unwrap())
        }
    }
}

#[derive(Deserialize)]
struct TokenPayload {
    token: String,
}

async fn healthcheck(State(state): State<Arc<AppState>>) -> StatusCode {
    let initialised = state.token.initialized();
    if !initialised {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::OK
}

fn check_token_exists(state: &Arc<AppState>, payload: &TokenPayload) -> bool {
    if let Some(existing_data) = state.token.get() {
        assert!(
            existing_data.token == payload.token,
            "already set up but provided with a different token"
        );
        return true;
    }
    false
}
fn filter_library_sources(
    library_sources: Vec<LibrarySource>,
) -> HashMap<String, (PathBuf, LibraryBackend)> {
    library_sources
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
        .collect()
}
fn set_generated_token(
    state: &Arc<AppState>,
    token: String,
    libraries: HashMap<String, (PathBuf, LibraryBackend)>,
) -> Result<(), StatusCode> {
    state
        .token
        .set(AppInitData { token, libraries })
        .map_err(|err| {
            error!("failed to set token: {err:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(())
}
