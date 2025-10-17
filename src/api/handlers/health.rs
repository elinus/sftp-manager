use crate::responses::api_response::ApiResponse;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
}

pub async fn health_check() -> ApiResponse<HealthResponse> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: None,
    };

    ApiResponse::success(response)
}
