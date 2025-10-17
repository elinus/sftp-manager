mod api;
mod config;
mod models;
mod services;

use crate::api::routes::configure_api_routes;
use crate::config::settings::Settings;
use axum::Router;
use std::net::SocketAddr;
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let settings = Settings::new().expect("Failed to load configuration");
    info!("Starting SFTP Manager API Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    let app = Router::new().merge(configure_api_routes());

    // Create the TCP listener
    let addr = SocketAddr::from(([0, 0, 0, 0], settings.server.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address.");
    info!("🚀 Server started successfully, listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error!");

    info!("Server stopped gracefully! 🧘");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received SIGTERM signal, shutting down gracefully...");
        },
    }

    // TODO: Add cleanup tasks here:
    // - Close database connections
    // - Stop SFTP server
}
