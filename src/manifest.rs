use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct DropChunk {
    pub permissions: usize,
    pub ids: Vec<String>,
    pub checksums: Vec<String>,
    pub lengths: Vec<usize>,
}

pub type DropletManifest = HashMap<String, DropChunk>;
