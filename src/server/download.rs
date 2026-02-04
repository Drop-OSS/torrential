use crate::{proto::{manifest::server_games_response::SkeletonGame, version::VersionResponse}, state::{AppState}, util::ErrorOption};

pub async fn fetch_version_data(
    app_state: &AppState,
    game_id: String,
    version_id: String,
) -> Result<VersionResponse, ErrorOption> {
    unreachable!();
}

pub async fn fetch_instance_games(
    app_state: &AppState,
) -> Result<Vec<SkeletonGame>, ErrorOption> {
    unreachable!()
}
