use std::{env, sync::LazyLock};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use droplet_rs::manifest::Manifest;
use log::info;
use reqwest::{Client, ClientBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{state::AppInitData, util::ErrorOption};

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionResponseBody {
    pub manifest: Manifest,
    pub library: LibrarySource,
    pub library_path: String,
    pub version_path: String,
}

#[derive(Serialize)]
pub struct VersionQuery {
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

pub async fn fetch_version_data(
    init_data: &AppInitData,
    game_id: String,
    version_id: String,
) -> Result<VersionResponseBody, ErrorOption> {
    let version_data_response = CLIENT
        .get(REMOTE_URL.join("/api/v1/admin/depot/manifest")?)
        .query(&VersionQuery {
            game: game_id,
            version: version_id,
        })
        .header("Authorization", format!("Bearer {}", init_data.key))
        .send()
        .await?;

    if !version_data_response.status().is_success() {
        if version_data_response.status() == StatusCode::BAD_REQUEST {
            return Err(StatusCode::NOT_FOUND.into());
        }

        return Err(anyhow!(
            "Fetching context failed with non-success code: {}, {}",
            version_data_response.status(),
            version_data_response
                .text()
                .await
                .unwrap_or("(failed to read body)".to_owned())
        )
        .into());
    }

    let version_data: VersionResponseBody = version_data_response.json().await?;

    Ok(version_data)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkeletonVersion {
    pub version_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkeletonGame {
    pub id: String,
    pub versions: Vec<SkeletonVersion>,
}

pub async fn fetch_instance_games(
    init_data: &AppInitData,
) -> Result<Vec<SkeletonGame>, ErrorOption> {
    let context_response = CLIENT
        .get(REMOTE_URL.join("/api/v1/admin/depot/versions")?)
        .header("Authorization", format!("Bearer {}", init_data.key))
        .send()
        .await?;

    if !context_response.status().is_success() {

        return Err(anyhow!(
            "Fetching instance games failed with non-success code: {}, {}",
            context_response.status(),
            context_response
                .text()
                .await
                .unwrap_or("(failed to read body)".to_owned())
        )
        .into());
    }

    let games: Vec<SkeletonGame> = context_response.json().await?;

    Ok(games)
}
