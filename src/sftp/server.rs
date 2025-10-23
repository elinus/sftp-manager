use crate::sftp::session::SshServerImpl;
use russh::keys::ssh_key::{self, rand_core::OsRng};
use russh::server::Server as _;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};

// Main SFTP server structure
#[derive(Clone)]
pub struct SftpServer {
    // Root directory path for the SFTP server
    pub root_dir: Arc<RwLock<String>>,
    // Optional credentials for authentication (username, password)
    pub credentials: Arc<RwLock<Option<(String, String)>>>,
}

impl SftpServer {
    // Creates a new SFTP server instance with the given root directory
    pub fn new(root_dir: String) -> Self {
        Self {
            root_dir: Arc::new(RwLock::new(root_dir)),
            credentials: Arc::new(RwLock::new(None)),
        }
    }

    // Sets the username/password credentials to be used for authentication
    pub async fn set_credentials(&self, username: String, password: String) {
        info!("Setting SFTP credentials for user: {}", username);
        let mut creds = self.credentials.write().await;
        *creds = Some((username, password));
    }

    // Clears the stored credentials
    #[allow(dead_code)]
    pub async fn clear_credentials(&self) {
        info!("Clearing SFTP credentials");
        let mut creds = self.credentials.write().await;
        *creds = None;
    }

    // Starts the SFTP server on the given address and port
    pub async fn start_server(
        self,
        addrs: String,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = create_ssh_config();
        let mut ssh_server = SshServerImpl::new(self);

        debug!("Starting SFTP server on Addrs:{}, Port: {}", addrs, port);

        ssh_server.run_on_address(Arc::new(config), (addrs, port)).await?;
        info!("SFTP server has shut down");
        Ok(())
    }
}

// Create SSH server configuration
fn create_ssh_config() -> russh::server::Config {
    russh::server::Config {
        auth_rejection_time: Duration::from_secs(3),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: vec![
            russh::keys::PrivateKey::random(
                &mut OsRng,
                ssh_key::Algorithm::Ed25519,
            )
            .expect("Failed to generate SSH key"),
        ],
        ..Default::default()
    }
}

// Entry point to run the SFTP server
// This is the main function called from the lifecycle manager
pub async fn run_sftp_server(
    root_dir: String,
    bind_address: String,
    port: u16,
    username: String,
    password: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Initializing SFTP server with root directory: {}", root_dir);

    let sftp_server = SftpServer::new(root_dir);
    sftp_server.set_credentials(username, password).await;

    info!("Starting SFTP server on {}:{}", bind_address, port);
    sftp_server.start_server(bind_address, port).await?;

    Ok(())
}
