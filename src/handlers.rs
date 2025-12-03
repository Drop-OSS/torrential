use std::sync::Arc;

use axum::{Json, extract::State};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::state::AppState;

pub async fn healthcheck(State(state): State<Arc<AppState>>) -> StatusCode {
    let initialised = state.token.initialized();
    if !initialised {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::OK
}

#[derive(Deserialize)]
pub struct InvalidateBody {
    game_id: String,
    version_name: String,
}

pub async fn invalidate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InvalidateBody>,
) -> StatusCode {
    state.context_cache.remove(&(payload.game_id, payload.version_name));
    StatusCode::OK
}
