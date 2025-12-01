use std::{env, sync::LazyLock};

use anyhow::{Result, anyhow};
use log::info;
use reqwest::{Client, ClientBuilder, StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::{manifest::DropletManifest, util::ErrorOption};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResponseBody {
    timeout: String,
    pub manifest: DropletManifest,
    version_name: String,
    pub library_id: String,
    pub library_path: String,
}

#[derive(Serialize)]
pub struct ContextQuery {
    game: String,
    version: String,
}

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    ClientBuilder::new()
        .build()
        .expect("failed to build client")
});

static REMOTE_URL: LazyLock<Url> = LazyLock::new(|| {
    let user_provided = env::var("DROP_SERVER_URL");
    let url = Url::parse(
        user_provided
            .as_ref()
            .map_or("http://localhost:3000", |v| v),
    )
    .expect("failed to parse URL");
    info!("using Drop server url {}", url);
    url
});

pub async fn fetch_download_context(
    token: String,
    game_id: String,
    version_name: String,
) -> Result<ContextResponseBody, ErrorOption> {
    let context_response = CLIENT
        .get(REMOTE_URL.join("/api/v1/admin/depot/context")?)
        .query(&ContextQuery {
            game: game_id,
            version: version_name,
        })
        .header("Authorization", format!("Bearer {}", token))
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
                .unwrap_or("(failed to read body)".to_string())
        ).into());
    }

    let context: ContextResponseBody = context_response.json().await?;

    Ok(context)
}


#[derive(Deserialize, Debug)]
#[non_exhaustive]
pub enum LibraryBackend {
    Filesystem,
    FlatFilesystem
}

#[derive(Deserialize)]
pub struct LibrarySource {
    pub options: serde_json::Value,
    pub id: String,
    name: String,
    pub backend: LibraryBackend
}

pub async fn fetch_library_sources(token: String) -> Result<Vec<LibrarySource>> {
    let source_response = CLIENT
        .get(REMOTE_URL.join("/api/v1/admin/library/sources")?)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !source_response.status().is_success() {
        return Err(anyhow!(
            "Fetching library sources failed with non-success code: {}, {}",
            source_response.status(),
            source_response
                .text()
                .await
                .unwrap_or("(failed to read body)".to_string())
        ));
    }

    let library_sources: Vec<LibrarySource> = source_response.json().await?;

    Ok(library_sources)
}