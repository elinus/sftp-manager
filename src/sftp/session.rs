use crate::sftp::handler::SftpSession;
use crate::sftp::server::SftpServer;
use russh::keys::ssh_key;
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Implements SSH server using russh
#[derive(Clone)]
pub struct SshServerImpl {
    pub sftp_server: SftpServer,
}

impl SshServerImpl {
    /// Create a new SSH server implementation
    pub fn new(sftp_server: SftpServer) -> Self {
        Self { sftp_server }
    }
}

impl russh::server::Server for SshServerImpl {
    type Handler = SshSession;

    fn new_client(&mut self, _addr: Option<SocketAddr>) -> Self::Handler {
        SshSession::new(self.sftp_server.clone())
    }
}

/// Represents a single SSH session and associated SFTP state
pub struct SshSession {
    /// Map of active client channels
    clients: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
    /// Reference to the parent SFTP server
    sftp_server: SftpServer,
}

impl SshSession {
    /// Create a new SSH session
    pub fn new(sftp_server: SftpServer) -> Self {
        Self { clients: Arc::new(Mutex::new(HashMap::new())), sftp_server }
    }

    /// Retrieves and removes a channel by ID from active clients
    async fn get_channel(&mut self, channel_id: ChannelId) -> Channel<Msg> {
        let mut clients = self.clients.lock().await;
        clients.remove(&channel_id).expect("Channel should exist")
    }
}

impl russh::server::Handler for SshSession {
    type Error = anyhow::Error;

    /// Handles password-based authentication
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<Auth, Self::Error> {
        info!("Auth attempt with password: user={}", user);

        let credentials = self.sftp_server.credentials.read().await;
        if let Some((username, pass)) = &*credentials
            && username == user
            && pass == password
        {
            info!("Authentication successful for user: {}", user);
            return Ok(Auth::Accept);
        }

        warn!("Authentication failed for user: {}", user);
        Ok(Auth::Reject { proceed_with_methods: None, partial_success: false })
    }

    /// Disables public key authentication
    async fn auth_publickey(
        &mut self,
        user: &str,
        _public_key: &ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        info!("Public key authentication attempt by {}, rejecting", user);
        Ok(Auth::Reject { proceed_with_methods: None, partial_success: false })
    }

    /// Handle channel EOF
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Channel EOF received, closing channel: {:?}", channel);
        session.close(channel)?;
        Ok(())
    }

    /// Handle a new channel session
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        debug!("Channel session opened: {:?}", channel.id());
        let mut clients = self.clients.lock().await;
        clients.insert(channel.id(), channel);
        Ok(true)
    }

    /// Handle subsystem requests (SFTP)
    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Subsystem request: {}", name);

        if name == "sftp" {
            let channel = self.get_channel(channel_id).await;
            let root_dir = self.sftp_server.root_dir.read().await.clone();

            session.channel_success(channel_id)?;
            info!("Starting SFTP subsystem with root directory: {}", root_dir);

            let sftp = SftpSession::new(root_dir);
            russh_sftp::server::run(channel.into_stream(), sftp).await;
        } else {
            warn!("Unsupported subsystem requested: {}", name);
            session.channel_failure(channel_id)?;
        }

        Ok(())
    }
}
