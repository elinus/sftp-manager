use crate::models::sftp::SftpState;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{error, info, warn};

/// SFTP lifecycle manager
/// Handles:
/// - Starting the SFTP server when enabled
/// - Stopping the server when disabled
/// - Checking for credential expiration
/// - Auto-disabling on expiration
pub struct SftpLifecycleManager {
    state: SftpState,
    bind_address: String,
    port: u16,
    root_directory: String,
    check_interval_secs: u64,
}

impl SftpLifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(
        state: SftpState,
        bind_address: String,
        port: u16,
        root_directory: String,
    ) -> Self {
        Self {
            state,
            bind_address,
            port,
            root_directory,
            check_interval_secs: 10, // Check every 10 seconds
        }
    }

    /// Start the lifecycle management task
    /// Returns a JoinHandle that can be used to stop the manager
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Main lifecycle loop
    async fn run(self) {
        info!("SFTP lifecycle manager started");

        let mut check_interval =
            interval(Duration::from_secs(self.check_interval_secs));
        let mut server_task: Option<JoinHandle<()>> = None;

        loop {
            // Wait for the next check
            check_interval.tick().await;

            // Check for expiration first
            if self.state.is_expired().await {
                warn!("SFTP credentials expired, disabling");
                self.state.disable().await;
            }

            let is_enabled = self.state.is_enabled().await;
            let is_running = server_task.is_some();

            match (is_enabled, is_running) {
                (true, false) => {
                    // Should be running but isn't - start it
                    info!("Starting SFTP server on port {}", self.port);

                    match self.start_server().await {
                        Ok(task) => {
                            server_task = Some(task);
                            info!("✅ SFTP server started successfully");
                        }
                        Err(e) => {
                            error!("❌ Failed to start SFTP server: {}", e);
                            // Disable on failure to prevent continuous restart attempts
                            self.state.disable().await;
                        }
                    }
                }
                (false, true) => {
                    // Should not be running but is - stop it
                    info!("Stopping SFTP server");

                    if let Some(task) = server_task.take() {
                        task.abort();
                        info!("✅ SFTP server stopped");
                    }
                }
                _ => {
                    // State is consistent, do nothing
                }
            }
        }
    }

    /// Start the actual SFTP server
    async fn start_server(
        &self,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
        // Get credentials
        let credentials = self
            .state
            .get_credentials()
            .await
            .ok_or("No credentials available")?;

        // Clone values for the task
        let bind_address = self.bind_address.clone();
        let port = self.port;
        let root_dir = self.root_directory.clone();
        let username = credentials.username.clone();
        let password = credentials.password.clone();

        info!(
            "Starting SFTP server: address={}, port={}, root={}, user={}",
            bind_address, port, root_dir, username
        );

        // Spawn the server task
        let task = tokio::spawn(async move {
            // Import the SFTP server run function
            use crate::sftp::run_sftp_server;

            info!("SFTP server task started");

            // Start the actual SFTP server
            if let Err(e) = run_sftp_server(
                root_dir,
                bind_address,
                port,
                username,
                password,
            )
            .await
            {
                error!("SFTP server error: {}", e);
            }

            info!("SFTP server task ended");
        });

        Ok(task)
    }
}

/// Convenience function to start the lifecycle manager
pub fn start_sftp_lifecycle(
    state: SftpState,
    bind_address: String,
    port: u16,
    root_directory: String,
) -> JoinHandle<()> {
    let manager =
        SftpLifecycleManager::new(state, bind_address, port, root_directory);

    manager.start()
}
