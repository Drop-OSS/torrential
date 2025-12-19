use std::{io::Error, rc::Rc, sync::Arc};

use aes::cipher::{KeyIvInit, StreamCipher};
use axum::{
    body::Body,
    extract::{Path, State},
    http::HeaderMap,
    response::{AppendHeaders, IntoResponse},
};
use bytes::Bytes;
use dashmap::{DashMap, mapref::one::RefMut};
use droplet_rs::{
    manifest::ChunkData,
    versions::types::{MinimumFileObject, VersionFile},
};
use futures_util::{StreamExt, stream};
use log::{error, info};
use reqwest::{StatusCode, header};
use tokio::sync::SemaphorePermit;
use tokio_util::io::ReaderStream;

use crate::{
    DownloadContext, GLOBAL_CONTEXT_SEMAPHORE, download::create_download_context, state::AppState,
};

type Aes128Ctr64LE = ctr::Ctr64LE<aes::Aes128>;

pub async fn serve_file(
    State(state): State<Arc<AppState>>,
    Path((game_id, version_name, chunk_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let context_cache = &state.context_cache;

    let mut context = get_or_create_context(&state, context_cache, game_id, version_name).await?;
    context.reset_last_access();

    let chunk_data = lookup_chunk(&chunk_id, &context)?;
    let mut streams = Vec::with_capacity(chunk_data.files.len());
    let mut content_length = 0;

    for file_entry in &chunk_data.files {
        let reader = get_file_reader(
            &mut context,
            file_entry.filename.clone(),
            file_entry.start,
            file_entry.start + file_entry.length,
        )
        .await?;

        let stream = ReaderStream::new(reader);
        streams.push(stream);
        content_length += file_entry.length;
    }

    let stream = stream::iter(streams).flatten();
    let mut cipher = Aes128Ctr64LE::new(&context.manifest.key.into(), &chunk_data.iv.into());
    let encrypted_stream = stream.chunks(3).map(move |raw| -> Result<Bytes, Error> {
        let data: Result<Vec<Bytes>, Error> = raw.into_iter().collect();
        let mut data = data?.concat();

        cipher.apply_keystream(&mut data);

        Ok(data.into())
    });
    let body: Body = Body::from_stream(encrypted_stream);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
    headers.insert("Content-Length", content_length.into());

    Ok((headers, body))
}
async fn acquire_permit<'a>() -> SemaphorePermit<'a> {
    return GLOBAL_CONTEXT_SEMAPHORE
        .acquire()
        .await
        .expect("failed to acquire semaphore");
}
/**
 * Needs to be cloned for reference reasons
 */
fn lookup_chunk(
    chunk_id: &str,
    context: &RefMut<'_, (String, String), DownloadContext>,
) -> Result<ChunkData, StatusCode> {
    context
        .manifest
        .chunks
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
            error!("reader error for '{}': {v:?}", relative_filename);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
async fn get_or_create_context<'a>(
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
            info!("generating context for {}...", game_id);
            let context_result =
                create_download_context(initialisation_data, game_id.clone(), version_name.clone())
                    .await?;

            state.context_cache.insert(key.clone(), context_result);

            info!("continuing download for {}", game_id);

            drop(permit);

            Ok(context_cache.get_mut(&key).unwrap())
        }
    }
}
