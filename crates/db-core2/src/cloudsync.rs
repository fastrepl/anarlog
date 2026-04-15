use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{ExponentialBuilder, Retryable};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite, SqlitePool};
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;

use crate::Db3;
use crate::pool::TableChange;


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
    pub force_init: Option<bool>,
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
    pub last_sync_downloaded_count: Option<i64>,
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
    fn normalized(mut self) -> Result<Self, CloudsyncRuntimeError> {
        if self.sync_interval_ms == 0 {
            return Err(CloudsyncRuntimeError::InvalidSyncInterval);
        }
        self.connection_string = self.connection_string.trim().to_string();
        Ok(self)
    }

    fn enabled_tables(&self) -> impl Iterator<Item = &CloudsyncTableSpec> {
        self.tables.iter().filter(|table| table.enabled)
    }
}

pub(crate) struct CloudsyncRuntimeState {
    pub(crate) config: Option<CloudsyncRuntimeConfig>,
    pub(crate) running: bool,
    pub(crate) network_initialized: bool,
    pub(crate) task: Option<CloudsyncBackgroundTask>,
    pub(crate) last_sync_downloaded_count: Option<i64>,
    pub(crate) last_sync_at_ms: Option<u64>,
    pub(crate) last_error: Option<String>,
    pub(crate) last_error_kind: Option<hypr_cloudsync::ErrorKind>,
    pub(crate) consecutive_failures: u32,
}

pub(crate) struct CloudsyncBackgroundTask {
    pub(crate) shutdown_tx: Option<oneshot::Sender<()>>,
    pub(crate) join_handle: JoinHandle<()>,
}

impl Default for CloudsyncRuntimeState {
    fn default() -> Self {
        Self {
            config: None,
            running: false,
            network_initialized: false,
            task: None,
            last_sync_downloaded_count: None,
            last_sync_at_ms: None,
            last_error: None,
            last_error_kind: None,
            consecutive_failures: 0,
        }
    }
}

impl std::fmt::Debug for CloudsyncRuntimeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudsyncRuntimeState")
            .field("config", &self.config)
            .field("running", &self.running)
            .field("network_initialized", &self.network_initialized)
            .field("has_task", &self.task.is_some())
            .field(
                "last_sync_downloaded_count",
                &self.last_sync_downloaded_count,
            )
            .field("last_sync_at_ms", &self.last_sync_at_ms)
            .field("last_error", &self.last_error)
            .field("last_error_kind", &self.last_error_kind)
            .field("consecutive_failures", &self.consecutive_failures)
            .finish()
    }
}

impl std::fmt::Debug for CloudsyncBackgroundTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudsyncBackgroundTask")
            .field("has_shutdown_tx", &self.shutdown_tx.is_some())
            .finish()
    }
}

impl Db3 {
    pub fn cloudsync_enabled(&self) -> bool {
        self.cloudsync_enabled
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&std::path::Path> {
        self.cloudsync_path.as_deref()
    }

    /// Subscribe to best-effort table-level change notifications for writes observed through
    /// this app's pooled SQLite connections.
    pub fn subscribe_table_changes(&self) -> broadcast::Receiver<TableChange> {
        self.pool.subscribe_table_changes()
    }

    pub fn cloudsync_configure(
        &self,
        config: CloudsyncRuntimeConfig,
    ) -> Result<(), CloudsyncRuntimeError> {
        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        if runtime.running {
            return Err(CloudsyncRuntimeError::RestartRequired);
        }
        runtime.config = Some(config.normalized()?);
        runtime.last_error = None;
        Ok(())
    }

    pub async fn cloudsync_reconfigure(
        &self,
        config: CloudsyncRuntimeConfig,
    ) -> Result<(), CloudsyncRuntimeError> {
        let was_running = self.cloudsync_runtime.lock().unwrap().running;

        if was_running {
            self.cloudsync_stop().await?;
        }

        self.cloudsync_configure(config)?;

        if was_running {
            self.cloudsync_start().await?;
        }

        Ok(())
    }

    pub async fn cloudsync_start(&self) -> Result<(), CloudsyncRuntimeError> {
        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.running = false;
            runtime.network_initialized = false;
            runtime.last_error = None;
            return Ok(());
        }

        let config = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            if runtime.running {
                return Ok(());
            }
            runtime
                .config
                .clone()
                .ok_or(CloudsyncRuntimeError::NotConfigured)?
        };

        for table in config.enabled_tables() {
            self.cloudsync_init(
                &table.table_name,
                table.crdt_algo.as_deref(),
                table.force_init,
            )
            .await?;
        }

        self.cloudsync_network_init(&config.connection_string)
            .await?;
        self.apply_cloudsync_auth(&config.auth).await?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let pool = self.pool.as_ref().clone();
        let runtime_state = Arc::clone(&self.cloudsync_runtime);
        let wait_ms = config.wait_ms;
        let max_retries = config.max_retries;
        let sync_interval_ms = config.sync_interval_ms;
        let join_handle = tokio::spawn(async move {
            cloudsync_background_loop(
                pool,
                runtime_state,
                sync_interval_ms,
                wait_ms,
                max_retries,
                shutdown_rx,
            )
            .await;
        });

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.network_initialized = true;
        runtime.last_error = None;
        runtime.last_error_kind = None;
        runtime.consecutive_failures = 0;
        runtime.task = Some(CloudsyncBackgroundTask {
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });

        Ok(())
    }

    pub async fn cloudsync_stop(&self) -> Result<(), CloudsyncRuntimeError> {
        let task = {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.running = false;
            runtime.task.take()
        };

        if let Some(mut task) = task {
            if let Some(shutdown_tx) = task.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            let _ = task.join_handle.await;
        }

        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.network_initialized = false;
            runtime.last_error = None;
            return Ok(());
        }

        let should_cleanup = self.cloudsync_runtime.lock().unwrap().network_initialized;
        if should_cleanup {
            self.cloudsync_network_cleanup().await?;
        }

        if self.has_cloudsync() {
            self.cloudsync_terminate().await?;
        }

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = false;
        runtime.last_error = None;
        Ok(())
    }

    pub async fn cloudsync_status(&self) -> Result<CloudsyncStatus, CloudsyncRuntimeError> {
        let (
            config,
            running,
            network_initialized,
            last_sync_downloaded_count,
            last_sync_at_ms,
            last_error,
            last_error_kind,
            consecutive_failures,
        ) = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            (
                runtime.config.clone(),
                runtime.running,
                runtime.network_initialized,
                runtime.last_sync_downloaded_count,
                runtime.last_sync_at_ms,
                runtime.last_error.clone(),
                runtime.last_error_kind.map(CloudsyncErrorKind::from),
                runtime.consecutive_failures,
            )
        };

        let has_unsent_changes =
            if self.cloudsync_enabled && network_initialized {
                Some(self.cloudsync_network_has_unsent_changes().await?)
            } else {
                None
            };

        Ok(CloudsyncStatus {
            cloudsync_enabled: self.cloudsync_enabled,
            extension_loaded: self.has_cloudsync(),
            configured: config.is_some(),
            running,
            network_initialized,
            last_sync_downloaded_count,
            last_sync_at_ms,
            has_unsent_changes,
            last_error,
            last_error_kind,
            consecutive_failures,
        })
    }

    pub async fn cloudsync_trigger_sync(&self) -> Result<i64, CloudsyncRuntimeError> {
        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_error = None;
            return Ok(0);
        }

        let (wait_ms, max_retries, network_initialized) = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            let config = runtime
                .config
                .as_ref()
                .ok_or(CloudsyncRuntimeError::NotConfigured)?;
            (
                config.wait_ms,
                config.max_retries,
                runtime.network_initialized,
            )
        };
        if !network_initialized {
            return Err(CloudsyncRuntimeError::NotStarted);
        }

        let downloaded_count = self.cloudsync_network_sync(wait_ms, max_retries).await?;
        self.record_cloudsync_sync_result(downloaded_count);
        Ok(downloaded_count)
    }

    pub async fn cloudsync_version(&self) -> Result<String, hypr_cloudsync::Error> {
        hypr_cloudsync::version(self.pool.as_ref()).await
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        force: Option<bool>,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::init(self.pool.as_ref(), table_name, crdt_algo, force).await
    }

    pub async fn cloudsync_network_init(
        &self,
        connection_string: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_init(self.pool.as_ref(), connection_string).await
    }

    pub async fn cloudsync_network_set_apikey(
        &self,
        api_key: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_set_apikey(self.pool.as_ref(), api_key).await
    }

    pub async fn cloudsync_network_set_token(
        &self,
        token: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_set_token(self.pool.as_ref(), token).await
    }

    pub async fn cloudsync_begin_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        cloudsync_begin_alter_on(self.pool.as_ref(), table_name).await
    }

    pub async fn cloudsync_commit_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        cloudsync_commit_alter_on(self.pool.as_ref(), table_name).await
    }

    pub async fn cloudsync_cleanup(&self, table_name: &str) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::cleanup(self.pool.as_ref(), table_name).await
    }

    pub async fn cloudsync_terminate(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::terminate(self.pool.as_ref()).await
    }

    pub async fn cloudsync_network_cleanup(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_cleanup(self.pool.as_ref()).await
    }

    pub async fn cloudsync_network_has_unsent_changes(
        &self,
    ) -> Result<bool, hypr_cloudsync::Error> {
        hypr_cloudsync::network_has_unsent_changes(self.pool.as_ref()).await
    }

    pub async fn cloudsync_network_send_changes(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<i64, hypr_cloudsync::Error> {
        hypr_cloudsync::network_send_changes(self.pool.as_ref(), wait_ms, max_retries).await
    }

    pub async fn cloudsync_network_check_changes(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<i64, hypr_cloudsync::Error> {
        hypr_cloudsync::network_check_changes(self.pool.as_ref(), wait_ms, max_retries).await
    }

    pub async fn cloudsync_network_reset_sync_version(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_reset_sync_version(self.pool.as_ref()).await
    }

    pub async fn cloudsync_network_logout(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_logout(self.pool.as_ref()).await
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<i64, hypr_cloudsync::Error> {
        hypr_cloudsync::network_sync(self.pool.as_ref(), wait_ms, max_retries).await
    }

    async fn apply_cloudsync_auth(
        &self,
        auth: &CloudsyncAuth,
    ) -> Result<(), hypr_cloudsync::Error> {
        match auth {
            CloudsyncAuth::None => Ok(()),
            CloudsyncAuth::ApiKey { api_key } => self.cloudsync_network_set_apikey(api_key).await,
            CloudsyncAuth::Token { token } => self.cloudsync_network_set_token(token).await,
        }
    }

    fn record_cloudsync_sync_result(&self, downloaded_count: i64) {
        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.last_sync_downloaded_count = Some(downloaded_count);
        runtime.last_sync_at_ms = Some(now_ms());
        runtime.last_error = None;
        runtime.last_error_kind = None;
        runtime.consecutive_failures = 0;
    }
}

pub async fn cloudsync_begin_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::begin_alter(executor, table_name).await
}

pub async fn cloudsync_commit_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::commit_alter(executor, table_name).await
}

const MAX_BACKOFF_SECS: u64 = 300;

async fn cloudsync_background_loop(
    pool: SqlitePool,
    runtime_state: Arc<Mutex<CloudsyncRuntimeState>>,
    sync_interval_ms: u64,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let base_interval = Duration::from_millis(sync_interval_ms);

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            _ = tokio::time::sleep(base_interval) => {
                let state = Arc::clone(&runtime_state);

                let result = (|| async {
                    hypr_cloudsync::network_sync(&pool, wait_ms, max_retries).await
                })
                    .retry(
                        ExponentialBuilder::default()
                            .with_min_delay(base_interval)
                            .with_max_delay(Duration::from_secs(MAX_BACKOFF_SECS))
                            .with_jitter(),
                    )
                    .when(|e| e.kind() == hypr_cloudsync::ErrorKind::Transient)
                    .notify(|e, dur| {
                        let mut runtime = state.lock().unwrap();
                        runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                        runtime.last_error = Some(e.to_string());
                        runtime.last_error_kind = Some(e.kind());
                        tracing::warn!(
                            error = %e,
                            retry_after = ?dur,
                            failures = runtime.consecutive_failures,
                            "cloudsync transient error, retrying",
                        );
                    })
                    .await;

                match result {
                    Ok(downloaded_count) => {
                        let mut runtime = runtime_state.lock().unwrap();
                        runtime.last_sync_downloaded_count = Some(downloaded_count);
                        runtime.last_sync_at_ms = Some(now_ms());
                        runtime.last_error = None;
                        runtime.last_error_kind = None;
                        runtime.consecutive_failures = 0;
                    }
                    Err(error) => {
                        let kind = error.kind();
                        let mut runtime = runtime_state.lock().unwrap();
                        runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                        runtime.last_error = Some(error.to_string());
                        runtime.last_error_kind = Some(kind);
                        runtime.running = false;
                        break;
                    }
                }
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
