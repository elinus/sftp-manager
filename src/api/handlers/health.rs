use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
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

#[derive(Debug)]
pub struct ApiResponse<T> {
    pub status: StatusCode,
    pub data: T,
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        (self.status, Json(self.data)).into_response()
    }
}

pub async fn health_check() -> ApiResponse<HealthResponse> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: None,
    };
    ApiResponse { status: StatusCode::OK, data: response }
}
