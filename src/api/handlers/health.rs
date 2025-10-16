use axum::{Json, http::StatusCode};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Response structure for health check
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status
    pub status: String,
    /// Current timestamp
    pub timestamp: String,
    /// Service version
    pub version: String,
    /// Uptime in seconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
}

pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: None, // Can be implemented with application state
    };

    (StatusCode::OK, Json(response))
}
