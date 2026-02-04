use std::{collections::HashMap, path::PathBuf, sync::Arc};

use dashmap::DashMap;
use tokio::sync::{OnceCell, Semaphore};

use crate::{DownloadContext, server::DropServer};

pub struct AppState {
    pub context_cache: DashMap<(String, String), DownloadContext>,
    pub server: Arc<DropServer>,
}
