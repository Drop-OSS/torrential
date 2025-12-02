use std::{collections::HashMap, hash::RandomState, time::Instant};

use droplet_rs::versions::{create_backend_constructor, types::VersionBackend};
use reqwest::StatusCode;

use crate::{
    remote::{ContextResponseBody, LibraryBackend, ContextProvider},
    state::AppInitData,
    util::ErrorOption,
};

pub struct DownloadContext {
    pub(crate) chunk_lookup_table: HashMap<String, (String, usize, usize)>,
    pub(crate) backend: Box<dyn VersionBackend + Send + Sync + 'static>,
    last_access: Instant,
}
impl DownloadContext {
    pub fn last_access(&self) -> Instant {
        self.last_access
    }
    pub fn reset_last_access(&mut self) {
        self.last_access = Instant::now()
    }
}

pub async fn create_download_context(
    metadata_provider: &dyn ContextProvider,
    backend_factory: &dyn BackendFactory,
    init_data: &AppInitData,
    game_id: String,
    version_name: String,
) -> Result<DownloadContext, ErrorOption> {
    let context = metadata_provider
        .fetch_context(init_data.token(), game_id, version_name.clone())
        .await?;

    let backend = backend_factory.create_backend(init_data, &context, &version_name)?;

    let mut chunk_lookup_table = HashMap::with_capacity_and_hasher(
        context.manifest.values().map(|v| v.ids.len()).sum(),
        RandomState::default(),
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

pub trait BackendFactory: Send + Sync {
    fn create_backend(
        &self,
        init_data: &AppInitData,
        context: &ContextResponseBody,
        version_name: &String,
    ) -> Result<Box<dyn VersionBackend + Send + Sync>, StatusCode>;
}

pub struct DropBackendFactory;
impl BackendFactory for DropBackendFactory {
    fn create_backend(
        &self,
        init_data: &AppInitData,
        context: &ContextResponseBody,
        version_name: &String,
    ) -> Result<Box<dyn VersionBackend + Send + Sync>, StatusCode> {
        let (version_path, backend) = init_data
            .libraries()
            .get(&context.library_id)
            .ok_or(StatusCode::NOT_FOUND)?;

        let version_path = version_path.join(&context.library_path);
        let version_path = match backend {
            LibraryBackend::Filesystem => version_path.join(version_name),
            LibraryBackend::FlatFilesystem => version_path,
        };

        let backend =
            create_backend_constructor(&version_path).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        // TODO: Not eat this error
        let backend = backend().map_err(|_e| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(backend)
    }
}
