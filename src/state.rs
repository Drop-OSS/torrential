use std::{collections::HashMap, path::PathBuf, sync::Arc};

use dashmap::DashMap;
use tokio::sync::OnceCell;

use crate::{
    BackendFactory, DownloadContext, LibraryConfigurationProvider, ContextProvider,
    remote::LibraryBackend,
};

pub struct AppState {
    pub token: OnceCell<AppInitData>,
    pub context_cache: DashMap<(String, String), DownloadContext>,

    pub metadata_provider: Arc<dyn ContextProvider>,
    pub backend_factory: Arc<dyn BackendFactory>,
    pub library_provider: Arc<dyn LibraryConfigurationProvider>,
}

#[derive(Debug)]
pub struct AppInitData {
    token: String,
    libraries: HashMap<String, (PathBuf, LibraryBackend)>,
}
impl AppInitData {
    pub fn new(token: String, libraries: HashMap<String, (PathBuf, LibraryBackend)>) -> Self {
        Self { token, libraries }
    }
    pub fn token(&self) -> String {
        self.token.clone()
    }
    pub fn set_token(&mut self, token: String) {
        self.token = token
    }
    pub fn libraries(&self) -> &HashMap<String, (PathBuf, LibraryBackend)> {
        &self.libraries
    }
}
