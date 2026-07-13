use serde::{Deserialize, Serialize};

pub use hypr_cloudsync::NetworkResult as CloudsyncNetworkResult;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CloudsyncAuth {
    None,
    ApiKey { api_key: String },
    Token { token: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncTableSpec {
    pub table_name: String,
    pub crdt_algo: Option<String>,
    pub init_flags: Option<i64>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncRuntimeConfig {
    pub connection_string: String,
    pub auth: CloudsyncAuth,
    pub tables: Vec<CloudsyncTableSpec>,
    pub sync_interval_ms: u64,
    pub wait_ms: Option<i64>,
    pub max_retries: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncErrorKind {
    Transient,
    Auth,
    Fatal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudsyncStatus {
    pub cloudsync_enabled: bool,
    pub extension_loaded: bool,
    pub configured: bool,
    pub running: bool,
    pub network_initialized: bool,
    pub last_sync: Option<CloudsyncNetworkResult>,
    pub last_sync_at_ms: Option<u64>,
    pub has_unsent_changes: Option<bool>,
    pub last_error: Option<String>,
    pub last_error_kind: Option<CloudsyncErrorKind>,
    pub consecutive_failures: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum CloudsyncRuntimeError {
    #[error("cloudsync runtime is not configured")]
    NotConfigured,
    #[error("cloudsync runtime is not started")]
    NotStarted,
    #[error("cloudsync runtime is running; stop it first or use cloudsync_reconfigure")]
    RestartRequired,
    #[error("cloudsync sync interval must be greater than 0")]
    InvalidSyncInterval,
    #[error("cloudsync has unsent local changes; sync first or explicitly discard them")]
    UnsentChanges,
    #[error(transparent)]
    Cloudsync(#[from] hypr_cloudsync::Error),
}

impl From<hypr_cloudsync::ErrorKind> for CloudsyncErrorKind {
    fn from(kind: hypr_cloudsync::ErrorKind) -> Self {
        match kind {
            hypr_cloudsync::ErrorKind::Transient => Self::Transient,
            hypr_cloudsync::ErrorKind::Auth => Self::Auth,
            hypr_cloudsync::ErrorKind::Fatal => Self::Fatal,
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

    pub(crate) fn enabled_tables(&self) -> impl Iterator<Item = &CloudsyncTableSpec> {
        self.tables.iter().filter(|table| table.enabled)
    }
}
