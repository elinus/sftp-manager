use crate::state::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
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

impl IntoResponse for HealthResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let uptime_diff = (Utc::now() - state.uptime).num_seconds() as u64;

    HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        version: "0.1.0".into(),
        uptime: Some(uptime_diff),
    }
}
