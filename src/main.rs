mod api;
mod config;
mod models;
mod services;

use crate::api::routes::configure_api_routes;
use axum::Router;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().merge(configure_api_routes());

    println!("🚀 Server started successfully");

    // Create the TCP listener
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app).await.expect("Server error!");

    println!("Server stopped gracefully");
    Ok(())
}
