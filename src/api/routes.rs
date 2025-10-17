use axum::{
    Router,
    routing::{get, post},
};

use crate::api::handlers;
use crate::api::handlers::health::health_check;
use crate::api::handlers::sftp::AppState;

pub fn configure_api_routes() -> Router {
    Router::new().route("/health", get(health_check))
}

pub fn configure_sftp_routes() -> Router<AppState> {
    Router::new()
        .route("/sftp/toggle", post(handlers::sftp::toggle_sftp))
        .route("/sftp/status", get(handlers::sftp::get_sftp_status))
        .route("/sftp/credentials", get(handlers::sftp::get_sftp_credentials))
}
