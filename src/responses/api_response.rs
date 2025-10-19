use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    #[serde(skip_serializing)]
    pub status: StatusCode,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    // Create a successful response with data
    pub fn success(data: T) -> Self {
        Self { status: StatusCode::OK, data: Some(data), message: None }
    }

    // Create an error response with a message and status
    pub fn error(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, data: None, message: Some(message.into()) }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}
