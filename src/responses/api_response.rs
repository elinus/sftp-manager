use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug)]
pub struct ApiResponse<T> {
    pub status: StatusCode,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    // Create a successful response with data
    pub fn success(data: T) -> Self {
        Self { status: StatusCode::OK, data: Some(data), message: None }
    }

    // Create an error response with message and status
    pub fn error(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, data: None, message: Some(message.into()) }
    }
}

impl<T> Default for ApiResponse<T> {
    fn default() -> Self {
        Self { status: StatusCode::OK, data: None, message: None }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let body = Json(json!({
            "status": self.status.as_u16(),
            "data": self.data,
            "message": self.message,
        }));
        (self.status, body).into_response()
    }
}
