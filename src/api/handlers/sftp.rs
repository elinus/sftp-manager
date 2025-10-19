use crate::models::sftp::{
    CredentialsResponse, SftpStatusResponse, ToggleSftpRequest,
    ToggleSftpResponse,
};
use crate::responses::api_response::ApiResponse;

use axum::Json;
use axum::extract::State;

use crate::state::AppState;
use tracing::info;

pub async fn toggle_sftp(
    State(state): State<AppState>,
    Json(payload): Json<Option<ToggleSftpRequest>>,
) -> ApiResponse<ToggleSftpResponse> {
    info!("Toggle SFTP request received");

    let expiration_days = payload.map(|p| p.expiration_days).unwrap_or(30);

    let response = state.sftp_service.toggle(expiration_days).await;

    info!("SFTP toggled: {}", response.status);
    response
}

pub async fn get_sftp_status(
    State(state): State<AppState>,
) -> ApiResponse<SftpStatusResponse> {
    info!("Get SFTP status request");
    state.sftp_service.get_status().await
}

pub async fn get_sftp_credentials(
    State(state): State<AppState>,
) -> Result<ApiResponse<CredentialsResponse>, ApiResponse<()>> {
    info!("Get SFTP credentials request");
    state.sftp_service.get_credentials().await
}
