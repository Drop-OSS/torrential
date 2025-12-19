use std::sync::Arc;

use axum::{Json, extract::State};
use log::{error, info};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::state::{AppInitData, AppState};

#[derive(Deserialize)]
pub struct TokenPayload {
    key: String,
}

pub async fn set_token(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TokenPayload>,
) -> Result<StatusCode, StatusCode> {
    if check_token_exists(&state, &payload) {
        return Ok(StatusCode::OK);
    }

    let key = payload.key;

    set_depot_key(&state, key)?;

    info!("connected to drop server successfully");

    Ok(StatusCode::OK)
}

fn check_token_exists(state: &Arc<AppState>, payload: &TokenPayload) -> bool {
    if let Some(existing_data) = state.token.get() {
        assert!(
            *existing_data.key == payload.key,
            "already set up but provided with a different token"
        );
        return true;
    }
    false
}

fn set_depot_key(
    state: &Arc<AppState>,
    key: String
) -> Result<(), StatusCode> {
    state
        .token
        .set(AppInitData { key })
        .map_err(|err| {
            error!("failed to set token: {err:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(())
}
