use crate::api::handlers::{self, health::health_check};
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn configure_health_routes() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

pub fn configure_sftp_routes() -> Router<AppState> {
    Router::new()
        .route("/sftp/toggle", post(handlers::sftp::toggle_sftp))
        .route("/sftp/status", get(handlers::sftp::get_sftp_status))
        .route("/sftp/credentials", get(handlers::sftp::get_sftp_credentials))
}
