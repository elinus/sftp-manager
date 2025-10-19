use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

// SFTP server state management
#[derive(Clone)]
pub struct SftpState {
    pub enabled: Arc<RwLock<bool>>,
    pub expiration: Arc<RwLock<Option<SystemTime>>>,
    pub credentials: Arc<RwLock<Option<SftpCredentials>>>,
    pub root_dir: Arc<RwLock<String>>,
}

impl SftpState {
    pub fn new(root_dir: String) -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            expiration: Arc::new(RwLock::new(None)),
            credentials: Arc::new(RwLock::new(None)),
            root_dir: Arc::new(RwLock::new(root_dir)),
        }
    }

    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    pub async fn enable(
        &self,
        credentials: SftpCredentials,
        expiration: Option<SystemTime>,
    ) {
        *self.enabled.write().await = true;
        *self.credentials.write().await = Some(credentials);
        *self.expiration.write().await = expiration;
    }

    pub async fn disable(&self) {
        *self.enabled.write().await = false;
        *self.credentials.write().await = None;
        *self.expiration.write().await = None;
    }

    pub async fn is_expired(&self) -> bool {
        if let Some(exp) = *self.expiration.read().await {
            SystemTime::now() >= exp
        } else {
            false
        }
    }

    pub async fn get_credentials(&self) -> Option<SftpCredentials> {
        self.credentials.read().await.clone()
    }

    pub async fn get_root_directory(&self) -> String {
        self.root_dir.read().await.clone()
    }
}

// SFTP credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpCredentials {
    pub username: String,
    pub password: String,
}

impl SftpCredentials {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
}

// Request to toggle SFTP server
#[derive(Debug, Deserialize)]
pub struct ToggleSftpRequest {
    /// Duration in seconds for credentials to be valid (optional)
    /// Default: 30 days
    #[serde(default = "default_expiration_days")]
    pub expiration_days: u64,
}

fn default_expiration_days() -> u64 {
    30
}

// Response when toggling SFTP
#[derive(Debug, Serialize)]
pub struct ToggleSftpResponse {
    pub status: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<SftpCredentials>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

// SFTP status response
#[derive(Debug, Serialize)]
pub struct SftpStatusResponse {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_seconds: Option<u64>,
}

/// Response for credentials endpoint
#[derive(Debug, Serialize)]
pub struct CredentialsResponse {
    pub username: String,
    pub password: String,
    pub bind_addrs: String,
    pub port: u16,
    pub root_dir: String,
}
