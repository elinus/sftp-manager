use crate::state::AppState;
use axum::{extract::State, response::IntoResponse};
use tracing::info;

pub async fn toggle_sftp(State(state): State<AppState>) -> impl IntoResponse {
    info!("🔁 Toggle SFTP request");
    let expiration_days = 30;
    state.sftp_service.toggle(expiration_days).await;
}

pub async fn get_sftp_status(
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("Get SFTP status request");
    state.sftp_service.get_status().await
}

pub async fn get_sftp_credentials(
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("Get SFTP credentials request");
    state.sftp_service.get_credentials().await
}
