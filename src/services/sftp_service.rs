use crate::models::sftp::{
    CredentialsResponse, SftpCredentials, SftpState, SftpStatusResponse,
    ToggleSftpResponse,
};
use crate::responses::sftp::SftpApiResponse;
use axum::http::StatusCode;
use rand::Rng;
use rand::distr::Alphanumeric;
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

// SFTP service for managing server lifecycle
pub struct SftpService {
    pub bind_addrs: String,
    pub port: u16,
    pub root_dir: String,
    pub state: SftpState,
}

impl SftpService {
    // Create a new SFTP service
    pub fn new(
        bind_addrs: String,
        port: u16,
        root_dir: String,
        sftp_state: SftpState,
    ) -> Self {
        Self { bind_addrs, port, root_dir, state: sftp_state }
    }

    // Toggle SFTP server on/off
    pub async fn toggle(&self) -> SftpApiResponse<ToggleSftpResponse> {
        let is_enabled = self.state.is_enabled().await;

        if is_enabled {
            // Disable SFTP
            info!("Disabling SFTP server");
            self.state.disable().await;

            SftpApiResponse::success(ToggleSftpResponse {
                status: "disabled".to_string(),
                enabled: false,
                credentials: None,
                expires_at: None,
            })
        } else {
            info!("Enabling SFTP server");

            // Generate new credentials
            let credentials = self.generate_credentials();

            // Calculate expiration time
            let expiration = Some(
                SystemTime::now() + Duration::from_secs(30 * 24 * 60 * 60),
            );

            // Enable the server
            self.state.enable(credentials.clone(), expiration).await;

            // Log formatted expiration date
            let formatted_expiration = expiration
                .map(format_system_time)
                .unwrap_or_else(|| "N/A".to_string());

            info!(
                "SFTP enabled with username: {}, expires at {}",
                credentials.username, formatted_expiration
            );

            SftpApiResponse::success(ToggleSftpResponse {
                status: "enabled".to_string(),
                enabled: true,
                credentials: Some(credentials),
                expires_at: expiration.map(format_system_time),
            })
        }
    }

    // Get current SFTP status
    pub async fn get_status(&self) -> SftpApiResponse<SftpStatusResponse> {
        let enabled = self.state.is_enabled().await;

        if !enabled {
            return SftpApiResponse::success(SftpStatusResponse {
                enabled: false,
                expires_at: None,
            });
        }

        // Check for expiration
        if self.state.is_expired().await {
            warn!("SFTP credentials have expired, disabling");
            self.state.disable().await;

            return SftpApiResponse::success(SftpStatusResponse {
                enabled: false,
                expires_at: None,
            });
        }

        // Get expiration info
        let expiration = *self.state.expiration.read().await;
        let expires_at = expiration.map(format_system_time);
        SftpApiResponse::success(SftpStatusResponse {
            enabled: true,
            expires_at,
        })
    }

    // Get SFTP credentials
    pub async fn get_credentials(
        &self,
    ) -> Result<SftpApiResponse<CredentialsResponse>, SftpApiResponse<()>> {
        // Check if enabled
        if !self.state.is_enabled().await {
            return Err(SftpApiResponse::error(
                StatusCode::BAD_REQUEST,
                "SFTP is not enabled".to_string(),
            ));
        }

        // Check if expired
        if self.state.is_expired().await {
            warn!("Attempted to get expired credentials");
            self.state.disable().await;
            return Err(SftpApiResponse::error(
                StatusCode::BAD_REQUEST,
                "SFTP credentials have expired".to_string(),
            ));
        }

        // Get credentials
        let credentials =
            self.state.get_credentials().await.ok_or_else(|| {
                SftpApiResponse::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No credentials found".to_string(),
                )
            })?;

        Ok(SftpApiResponse::success(CredentialsResponse {
            username: credentials.username,
            password: credentials.password,
            root_dir: self.root_dir.clone(),
            bind_addrs: self.bind_addrs.clone(),
            port: self.port,
        }))
    }

    /// Generate random credentials
    fn generate_credentials(&self) -> SftpCredentials {
        let username: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();

        let password: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        SftpCredentials::new(username, password)
    }

    // Check and handle expiration
    #[allow(dead_code)]
    pub async fn check_expiration(&self) -> bool {
        if self.state.is_expired().await {
            info!("SFTP credentials expired, disabling server");
            self.state.disable().await;
            true
        } else {
            false
        }
    }
}

// Format SystemTime
fn format_system_time(time: SystemTime) -> String {
    let duration =
        time.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();

    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}
