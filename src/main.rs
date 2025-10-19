mod api;
mod config;
mod models;
mod responses;
mod services;
mod state;
mod utils;

use crate::api::routes::{configure_health_routes, configure_sftp_routes};
use crate::config::settings::Settings;
use crate::models::sftp::SftpState;
use crate::services::sftp::SftpService;
use crate::utils::logger::init_logging;
use axum::Router;
use chrono::Utc;
use state::AppState;
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let settings = Settings::new().expect("Failed to load configuration");
    info!("Starting SFTP Manager API Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Initialize SFTP state
    let sftp_root = settings.sftp.root_dir.clone();
    let sftp_port = settings.sftp.port;
    let sftp_state = SftpState::new(sftp_root.clone());
    let sftp_service =
        Arc::new(SftpService::new(sftp_state.clone(), sftp_port));
    let app_state = AppState { sftp_service, uptime: Utc::now() };

    let app = Router::new()
        .merge(configure_health_routes())
        .merge(configure_sftp_routes())
        .with_state(app_state.clone());

    // Create the TCP listener
    let addr = SocketAddr::from(([0, 0, 0, 0], settings.server.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address.");
    info!("🚀 Server started successfully, listening on http://{}", addr);

    axum::serve(listener, app.into_make_service())
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
