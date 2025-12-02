use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    response::{AppendHeaders, IntoResponse},
};
use dashmap::{DashMap, mapref::one::RefMut};
use droplet_rs::versions::types::{MinimumFileObject, VersionFile};
use log::{error, info};
use reqwest::{StatusCode, header};
use tokio::sync::SemaphorePermit;
use tokio_util::io::ReaderStream;

use crate::{
    DownloadContext, GLOBAL_CONTEXT_SEMAPHORE, download::create_download_context, state::AppState,
};

pub async fn serve_file(
    State(state): State<Arc<AppState>>,
    Path((game_id, version_name, chunk_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let context_cache = &state.context_cache;

    let mut context = get_or_generate_context(&state, context_cache, game_id, version_name).await?;
    context.reset_last_access();

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

pub async fn healthcheck(State(state): State<Arc<AppState>>) -> StatusCode {
    let initialised = state.token.initialized();
    if !initialised {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::OK
}

async fn acquire_permit<'a>() -> SemaphorePermit<'a> {
    return GLOBAL_CONTEXT_SEMAPHORE
        .acquire()
        .await
        .expect("failed to acquire semaphore");
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
            let context_result = create_download_context(
                &*state.metadata_provider,
                &*state.backend_factory,
                initialisation_data,
                game_id.clone(),
                version_name.clone(),
            )
            .await?;

            state.context_cache.insert(key.clone(), context_result);

            info!("continuing download");

            drop(permit);

            Ok(context_cache.get_mut(&key).unwrap())
        }
    }
}
