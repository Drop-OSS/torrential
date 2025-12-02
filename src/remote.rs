use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{manifest::DropletManifest, util::ErrorOption};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResponseBody {
    pub manifest: DropletManifest,
    pub library_id: String,
    pub library_path: String,
}

#[derive(Serialize)]
pub struct ContextQuery {
    game: String,
    version: String,
}

#[derive(Deserialize, Debug)]
#[non_exhaustive]
pub enum LibraryBackend {
    Filesystem,
    FlatFilesystem,
}

#[derive(Deserialize)]
pub struct LibrarySource {
    pub options: serde_json::Value,
    pub id: String,
    pub backend: LibraryBackend,
}

pub struct DropContextProvider {
    client: reqwest::Client,
    base_url: Url,
}
impl DropContextProvider {
    pub fn new(url: Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: url,
        }
    }
}
#[async_trait]
impl ContextProvider for DropContextProvider {
    async fn fetch_context(
        &self,
        token: String,
        game_id: String,
        version_name: String,
    ) -> Result<ContextResponseBody, ErrorOption> {
        let context_response = self
            .client
            .get(self.base_url.join("/api/v1/admin/depot/context")?)
            .query(&ContextQuery {
                game: game_id,
                version: version_name,
            })
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await?;

        if !context_response.status().is_success() {
            if context_response.status() == StatusCode::BAD_REQUEST {
                return Err(StatusCode::NOT_FOUND.into());
            }

            return Err(anyhow!(
                "Fetching context failed with non-success code: {}, {}",
                context_response.status(),
                context_response
                    .text()
                    .await
                    .unwrap_or("(failed to read body)".to_owned())
            )
            .into());
        }

        let context: ContextResponseBody = context_response.json().await?;

        Ok(context)
    }
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    /// Fetches the manifest for a specific game version.
    async fn fetch_context(
        &self,
        token: String,
        game_id: String,
        version_name: String,
    ) -> Result<ContextResponseBody, ErrorOption>;
}

#[async_trait]
pub trait LibraryConfigurationProvider: Send + Sync {
    async fn fetch_sources(&self, token: &String) -> anyhow::Result<Vec<LibrarySource>>;
}
pub struct DropLibraryProvider {
    client: reqwest::Client,
    base_url: Url,
}
impl DropLibraryProvider {
    pub fn new(url: Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: url,
        }
    }
}

#[async_trait]
impl LibraryConfigurationProvider for DropLibraryProvider {
    async fn fetch_sources(&self, token: &String) -> anyhow::Result<Vec<LibrarySource>> {
        let source_response = self
            .client
            .get(self.base_url.join("/api/v1/admin/library/sources")?)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await?;

        if !source_response.status().is_success() {
            return Err(anyhow!(
                "Fetching library sources failed with non-success code: {}, {}",
                source_response.status(),
                source_response
                    .text()
                    .await
                    .unwrap_or("(failed to read body)".to_owned())
            ));
        }

        let library_sources: Vec<LibrarySource> = source_response.json().await?;

        Ok(library_sources)
    }
}
