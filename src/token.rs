use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc};

use axum::{Json, extract::State};
use log::{error, info};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::remote::{self, LibraryBackend, LibrarySource};
use crate::state::{AppInitData, AppState};

#[derive(Deserialize)]
pub struct TokenPayload {
    token: String,
}

pub async fn set_token(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TokenPayload>,
) -> Result<StatusCode, StatusCode> {
    if check_token_exists(&state, &payload) {
        return Ok(StatusCode::OK);
    }

    let token = payload.token;

    let library_sources = state
        .library_provider
        .fetch_sources(&token)
        .await
        .map_err(|v| {
            error!("{v:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let valid_library_sources = filter_library_sources(library_sources);

    set_generated_token(&state, token, valid_library_sources)?;

    info!("connected to drop server successfully");

    Ok(StatusCode::OK)
}

fn check_token_exists(state: &Arc<AppState>, payload: &TokenPayload) -> bool {
    if let Some(existing_data) = state.token.get() {
        assert!(
            *existing_data.token() == payload.token,
            "already set up but provided with a different token"
        );
        return true;
    }
    false
}
fn filter_library_sources(
    library_sources: Vec<LibrarySource>,
) -> HashMap<String, (PathBuf, LibraryBackend)> {
    library_sources
        .into_iter()
        .filter(|v| {
            matches!(
                v.backend,
                remote::LibraryBackend::Filesystem | remote::LibraryBackend::FlatFilesystem
            )
        })
        .map(|v| {
            let path = PathBuf::from_str(
                v.options
                    .as_object()
                    .unwrap()
                    .get("baseDir")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap();

            (v.id, (path, v.backend))
        })
        .collect()
}
fn set_generated_token(
    state: &Arc<AppState>,
    token: String,
    libraries: HashMap<String, (PathBuf, LibraryBackend)>,
) -> Result<(), StatusCode> {
    state
        .token
        .set(AppInitData::new(token, libraries))
        .map_err(|err| {
            error!("failed to set token: {err:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(())
}
