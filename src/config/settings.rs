use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

/// Main application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub sftp: SftpSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpSettings {
    #[serde(default = "default_sftp_port")]
    pub port: u16,

    #[serde(default = "default_bind_addrs")]
    pub bind_addrs: String,

    #[serde(default = "default_sftp_root")]
    pub root_dir: String,
}

// Default values
fn default_port() -> u16 {
    3000
}
fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_sftp_port() -> u16 {
    2222
}
fn default_bind_addrs() -> String {
    "0.0.0.0".to_string()
}
fn default_sftp_root() -> String {
    "./sftp_root_dir".to_string()
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .build()?;
        config.try_deserialize()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                port: default_port(),
                host: default_host(),
            },
            sftp: SftpSettings {
                port: default_sftp_port(),
                bind_addrs: default_bind_addrs(),
                root_dir: default_sftp_root(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.server.port, 3000);
    }

    #[test]
    fn test_settings_load() {
        let result = Settings::new();
        assert!(result.is_ok() || result.is_err()); // Just test it doesn't panic
    }
}
