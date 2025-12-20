use std::{collections::HashMap, path::PathBuf, sync::Arc};

use dashmap::DashMap;
use tokio::sync::{OnceCell, Semaphore};

use crate::
 DownloadContext
;

pub struct AppState {
    pub token: OnceCell<AppInitData>,
    pub context_cache: DashMap<(String, String), DownloadContext>,
}

#[derive(Debug)]
pub struct AppInitData {
    pub key: String,
}