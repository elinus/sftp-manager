use crate::services::sftp::SftpService;
use chrono::{DateTime, Utc};
use std::sync::Arc;

// Application state containing SFTP service
#[derive(Clone)]
pub struct AppState {
    pub sftp_service: Arc<SftpService>,
    pub uptime: DateTime<Utc>,
}
