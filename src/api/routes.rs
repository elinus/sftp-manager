use axum::{Router, routing::get};

use crate::api::handlers::health::health_check;

pub fn configure_api_routes() -> Router {
    Router::new().route("/health", get(health_check))
}
