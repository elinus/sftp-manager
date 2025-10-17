use crate::models::sftp::{
    CredentialsResponse, SftpCredentials, SftpState, SftpStatusResponse,
    ToggleSftpResponse,
};
use crate::responses::api_response::ApiResponse;
use axum::http::StatusCode;
use rand::Rng;
use rand::distr::Alphanumeric;
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

// SFTP service for managing server lifecycle
pub struct SftpService {
    state: SftpState,
    port: u16,
}

impl SftpService {
    // Create a new SFTP service
    pub fn new(state: SftpState, port: u16) -> Self {
        Self { state, port }
    }

    // Toggle SFTP server on/off
    pub async fn toggle(
        &self,
        expiration_days: u64,
    ) -> ApiResponse<ToggleSftpResponse> {
        let is_enabled = self.state.is_enabled().await;

        if is_enabled {
            // Disable SFTP
            info!("Disabling SFTP server");
            self.state.disable().await;

            ApiResponse::success(ToggleSftpResponse {
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
            let expiration = if expiration_days > 0 {
                Some(
                    SystemTime::now()
                        + Duration::from_secs(expiration_days * 24 * 60 * 60),
                )
            } else {
                None
            };

            // Enable the server
            self.state.enable(credentials.clone(), expiration).await;

            info!(
                "SFTP enabled with username: {}, expires in {} days",
                credentials.username, expiration_days
            );

            ApiResponse::success(ToggleSftpResponse {
                status: "enabled".to_string(),
                enabled: true,
                credentials: Some(credentials),
                expires_at: expiration.map(|e| format_system_time(e)),
            })
        }
    }

    // Get current SFTP status
    pub async fn get_status(&self) -> ApiResponse<SftpStatusResponse> {
        let enabled = self.state.is_enabled().await;
        let root_directory = self.state.get_root_directory().await;

        if !enabled {
            return ApiResponse::success(SftpStatusResponse {
                enabled: false,
                root_directory,
                expires_at: None,
                expires_in_seconds: None,
            });
        }

        // Check for expiration
        if self.state.is_expired().await {
            warn!("SFTP credentials have expired, disabling");
            self.state.disable().await;

            return ApiResponse::success(SftpStatusResponse {
                enabled: false,
                root_directory,
                expires_at: None,
                expires_in_seconds: None,
            });
        }

        // Get expiration info
        let expiration = *self.state.expiration.read().await;
        let (expires_at, expires_in_seconds) = if let Some(exp) = expiration {
            let expires_at = format_system_time(exp);
            let expires_in = exp
                .duration_since(SystemTime::now())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (Some(expires_at), Some(expires_in))
        } else {
            (None, None)
        };

        ApiResponse::success(SftpStatusResponse {
            enabled: true,
            root_directory,
            expires_at,
            expires_in_seconds,
        })
    }

    // Get SFTP credentials
    pub async fn get_credentials(
        &self,
    ) -> Result<ApiResponse<CredentialsResponse>, ApiResponse<()>> {
        // Check if enabled
        if !self.state.is_enabled().await {
            return Err(ApiResponse::error(
                StatusCode::BAD_REQUEST,
                "SFTP is not enabled".to_string(),
            ));
        }

        // Check if expired
        if self.state.is_expired().await {
            warn!("Attempted to get expired credentials");
            self.state.disable().await;
            return Err(ApiResponse::error(
                StatusCode::BAD_REQUEST,
                "SFTP credentials have expired".to_string(),
            ));
        }

        // Get credentials
        let credentials =
            self.state.get_credentials().await.ok_or_else(|| {
                ApiResponse::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No credentials found".to_string(),
                )
            })?;

        let root_directory = self.state.get_root_directory().await;
        Ok(ApiResponse::success(CredentialsResponse {
            username: credentials.username,
            password: credentials.password,
            root_directory,
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
