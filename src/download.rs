use std::{collections::HashMap, path::PathBuf, time::Instant};

use droplet_rs::versions::{create_backend_constructor, types::VersionBackend};
use reqwest::StatusCode;

use crate::{
    AppInitData, DownloadContext,
    remote::{ContextResponseBody, LibraryBackend, fetch_download_context},
    util::ErrorOption,
};

pub async fn create_download_context<'a>(
    init_data: &AppInitData,
    game_id: String,
    version_name: String,
) -> Result<DownloadContext, ErrorOption> {
    let context =
        fetch_download_context(init_data.token.clone(), game_id, version_name.clone()).await?;

    let backend = generate_backend(init_data, &context, &version_name)??;

    let mut chunk_lookup_table = HashMap::with_capacity_and_hasher(
        context.manifest.values().map(|v| v.ids.len()).sum(),
        Default::default(),
    );

    for (path, file_chunks) in context.manifest {
        let mut start = 0;
        for (chunk, length) in file_chunks.ids.into_iter().zip(file_chunks.lengths) {
            chunk_lookup_table.insert(chunk, (path.clone(), start, start + length));
            start += length;
        }
    }

    let download_context = DownloadContext {
        chunk_lookup_table,
        backend,
        last_access: Instant::now(),
    };

    Ok(download_context)
}

fn generate_backend(
    init_data: &AppInitData,
    context: &ContextResponseBody,
    version_name: &String,
) -> Result<Result<Box<dyn VersionBackend + Send + Sync>, anyhow::Error>, StatusCode> {
    let (version_path, backend) = init_data
        .libraries
        .get(&context.library_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let version_path = version_path.join(&context.library_path);
    let version_path = match backend {
        LibraryBackend::Filesystem => version_path.join(version_name),
        LibraryBackend::FlatFilesystem => version_path,
    };
    
    let backend =
        create_backend_constructor(&version_path).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let backend = backend();
    Ok(backend)
}
