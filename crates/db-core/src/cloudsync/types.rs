use serde::{Deserialize, Serialize};

pub use anlg_cloudsync::CloudsyncTableSpec;
pub use anlg_cloudsync::NetworkResult as CloudsyncNetworkResult;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CloudsyncAuth {
    None,
    ApiKey { api_key: String },
    Token { token: String },
}

impl std::fmt::Debug for CloudsyncAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::ApiKey { .. } => f
                .debug_struct("ApiKey")
                .field("api_key", &"[REDACTED]")
                .finish(),
            Self::Token { .. } => f
                .debug_struct("Token")
                .field("token", &"[REDACTED]")
                .finish(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncRuntimeConfig {
    pub connection_string: String,
    pub auth: CloudsyncAuth,
    pub tables: Vec<CloudsyncTableSpec>,
    pub sync_interval_ms: u64,
    pub wait_ms: Option<i64>,
    pub max_retries: Option<i64>,
}

impl std::fmt::Debug for CloudsyncRuntimeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudsyncRuntimeConfig")
            .field("connection_string", &"[REDACTED]")
            .field("auth", &self.auth)
            .field("tables", &self.tables)
            .field("sync_interval_ms", &self.sync_interval_ms)
            .field("wait_ms", &self.wait_ms)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncErrorKind {
    Transient,
    Auth,
    Fatal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncActivityTrigger {
    Background,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncActivityStatus {
    Completed,
    Progress,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncActivityEntry {
    pub timestamp_ms: u64,
    pub trigger: CloudsyncActivityTrigger,
    pub status: CloudsyncActivityStatus,
    pub sent_bytes: u64,
    pub received_bytes: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncStatus {
    pub cloudsync_enabled: bool,
    pub extension_loaded: bool,
    pub configured: bool,
    pub running: bool,
    pub network_initialized: bool,
    pub activity_paused: bool,
    pub last_sync: Option<CloudsyncNetworkResult>,
    pub last_sync_at_ms: Option<u64>,
    pub has_unsent_changes: Option<bool>,
    pub last_error: Option<String>,
    pub last_error_kind: Option<CloudsyncErrorKind>,
    pub consecutive_failures: u32,
    pub activity_log: Vec<CloudsyncActivityEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum CloudsyncRuntimeError {
    #[error("cloudsync runtime is unavailable in local-only mode")]
    Unavailable,
    #[error("cloudsync runtime is not configured")]
    NotConfigured,
    #[error("cloudsync runtime is not started")]
    NotStarted,
    #[error("cloudsync runtime is active; stop it first or use cloudsync_reconfigure")]
    RestartRequired,
    #[error("cloudsync sync interval must be greater than 0")]
    InvalidSyncInterval,
    #[error("cloudsync has unsent local changes; sync first or explicitly discard them")]
    UnsentChanges,
    #[error("cloudsync local status is busy; try again")]
    LocalStatusBusy,
    #[error(transparent)]
    Cloudsync(#[from] anlg_cloudsync::Error),
}

impl CloudsyncRuntimeError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Cloudsync(error) if error.kind() == anlg_cloudsync::ErrorKind::Transient
        )
    }
}

impl From<anlg_cloudsync::ErrorKind> for CloudsyncErrorKind {
    fn from(kind: anlg_cloudsync::ErrorKind) -> Self {
        match kind {
            anlg_cloudsync::ErrorKind::Transient => Self::Transient,
            anlg_cloudsync::ErrorKind::Auth => Self::Auth,
            anlg_cloudsync::ErrorKind::Fatal => Self::Fatal,
        }
    }
}

impl CloudsyncRuntimeConfig {
    pub(crate) fn normalized(mut self) -> Result<Self, CloudsyncRuntimeError> {
        if self.sync_interval_ms == 0 {
            return Err(CloudsyncRuntimeError::InvalidSyncInterval);
        }
        self.connection_string = self.connection_string.trim().to_string();
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_redacts_cloudsync_credentials() {
        let config = CloudsyncRuntimeConfig {
            connection_string: "sqlitecloud://project/database?apikey=connection-secret"
                .to_string(),
            auth: CloudsyncAuth::Token {
                token: "token-secret".to_string(),
            },
            tables: Vec::new(),
            sync_interval_ms: 30_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        };
        let debug = format!("{config:?}");

        assert!(!debug.contains("connection-secret"));
        assert!(!debug.contains("token-secret"));
        assert!(debug.contains("[REDACTED]"));

        let api_key = format!(
            "{:?}",
            CloudsyncAuth::ApiKey {
                api_key: "api-key-secret".to_string(),
            }
        );
        assert!(!api_key.contains("api-key-secret"));
        assert!(api_key.contains("[REDACTED]"));
    }

    #[test]
    fn runtime_exposes_transient_cloudsync_errors() {
        let error = CloudsyncRuntimeError::from(anlg_cloudsync::Error::from(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "connection reset",
        )));

        assert!(error.is_transient());
        assert!(!CloudsyncRuntimeError::NotStarted.is_transient());
    }
}
