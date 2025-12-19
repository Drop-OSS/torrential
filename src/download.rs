use std::{path::PathBuf, time::Instant};

use anyhow::anyhow;
use droplet_rs::{
    manifest::Manifest,
    versions::{create_backend_constructor, types::VersionBackend},
};
use log::{info, warn};
use reqwest::StatusCode;

use crate::{
    remote::{LibraryBackend, VersionResponseBody, fetch_version_data},
    state::AppInitData,
    util::ErrorOption,
};

pub struct DownloadContext {
    pub(crate) manifest: Manifest,
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
    init_data: &AppInitData,
    game_id: String,
    version_name: String,
) -> Result<DownloadContext, ErrorOption> {
    let version_data = fetch_version_data(init_data, game_id, version_name.clone()).await?;

    let backend = create_backend(&version_data)?;

    let download_context = DownloadContext {
        manifest: version_data.manifest,
        backend,
        last_access: Instant::now(),
    };

    Ok(download_context)
}

fn create_backend(
    version_data: &VersionResponseBody,
) -> Result<Box<dyn VersionBackend + Send + Sync>, StatusCode> {
    let base_path = version_data
        .library
        .options
        .get("baseDir")
        .unwrap()
        .as_str()
        .unwrap();

    let version_path = PathBuf::from(base_path);
    let version_path = version_path.join(version_data.library_path.clone());
    let version_path = match version_data.library.backend {
        LibraryBackend::Filesystem => version_path.join(version_data.version_path.clone()),
        LibraryBackend::FlatFilesystem => version_path,
    };

    if !version_path.exists() {
        warn!("{} path doesn't exist for version", version_path.display());
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let backend =
        create_backend_constructor(&version_path).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let backend = backend()
        .inspect_err(|err| warn!("failed to create version backend: {:?}", err))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(backend)
}
