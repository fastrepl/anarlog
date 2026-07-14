use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{ExponentialBuilder, Retryable};
use sqlx::pool::PoolConnection;
use sqlx::{Sqlite, SqlitePool};
use tokio::sync::oneshot;

use super::state::{CloudsyncBackgroundTask, CloudsyncRuntimeState};
use super::types::{
    CloudsyncErrorKind, CloudsyncNetworkResult, CloudsyncRuntimeConfig, CloudsyncRuntimeError,
    CloudsyncStatus,
};
use crate::Db;

impl Db {
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
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        let needs_cleanup = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            !runtime.running && (runtime.network_initialized || runtime.task.is_some())
        };
        if needs_cleanup {
            self.cloudsync_stop_locked().await?;
        }

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
            if let Err(error) = self
                .cloudsync_init(
                    &table.table_name,
                    table.crdt_algo.as_deref(),
                    table.init_flags,
                )
                .await
            {
                self.cleanup_failed_cloudsync_start(false).await;
                return Err(error.into());
            }
        }

        if let Err(error) = self.cloudsync_network_init(&config.connection_string).await {
            self.cleanup_failed_cloudsync_start(true).await;
            return Err(error.into());
        }
        if let Err(error) = self.apply_cloudsync_auth(&config.auth).await {
            self.cleanup_failed_cloudsync_start(true).await;
            return Err(error.into());
        }
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let pool = self.pool.clone();
        let connection = Arc::clone(&self.cloudsync_connection);
        let runtime_state = Arc::clone(&self.cloudsync_runtime);
        let wait_ms = config.wait_ms;
        let max_retries = config.max_retries;
        let sync_interval_ms = config.sync_interval_ms;
        let join_handle = tokio::spawn(async move {
            cloudsync_background_loop(
                pool,
                connection,
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
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.cloudsync_stop_locked().await
    }

    async fn cloudsync_stop_locked(&self) -> Result<(), CloudsyncRuntimeError> {
        let should_cleanup = self.stop_cloudsync_task().await;

        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.network_initialized = false;
            runtime.last_error = None;
            return Ok(());
        }

        if should_cleanup {
            self.cloudsync_network_cleanup().await?;
        }

        if self.has_cloudsync() {
            self.cloudsync_terminate().await?;
        }
        self.cloudsync_connection.lock().await.take();

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = false;
        runtime.last_error = None;
        Ok(())
    }

    pub async fn cloudsync_logout(
        &self,
        discard_unsent_changes: bool,
    ) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        let network_initialized = self.cloudsync_runtime.lock().unwrap().network_initialized;

        if !self.cloudsync_enabled {
            self.cloudsync_runtime.lock().unwrap().config = None;
            return Ok(());
        }

        let has_unsent_changes =
            network_initialized && self.cloudsync_network_has_unsent_changes().await?;
        if has_unsent_changes && !discard_unsent_changes {
            return Err(CloudsyncRuntimeError::UnsentChanges);
        }

        self.stop_cloudsync_task().await;
        let logout_result = if network_initialized {
            self.cloudsync_network_logout().await
        } else {
            Ok(())
        };
        let cleanup_result = self.cloudsync_network_cleanup().await;
        let terminate_result = if self.has_cloudsync() {
            self.cloudsync_terminate().await
        } else {
            Ok(())
        };
        self.cloudsync_connection.lock().await.take();

        let logout_error = logout_result
            .as_ref()
            .err()
            .map(|error| (error.to_string(), error.kind()));

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = false;
        if let Some((error, kind)) = logout_error {
            runtime.last_error = Some(error);
            runtime.last_error_kind = Some(kind);
        } else {
            runtime.config = None;
            runtime.last_sync = None;
            runtime.last_sync_at_ms = None;
            runtime.last_error = None;
            runtime.last_error_kind = None;
            runtime.consecutive_failures = 0;
        }
        drop(runtime);

        logout_result?;
        if network_initialized {
            cleanup_result?;
        } else if let Err(error) = cleanup_result {
            tracing::warn!(%error, "cloudsync cleanup after partial startup failed");
        }
        terminate_result?;
        Ok(())
    }

    pub async fn cloudsync_status(&self) -> Result<CloudsyncStatus, CloudsyncRuntimeError> {
        let (
            config,
            running,
            network_initialized,
            last_sync,
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
                runtime.last_sync.clone(),
                runtime.last_sync_at_ms,
                runtime.last_error.clone(),
                runtime.last_error_kind.map(CloudsyncErrorKind::from),
                runtime.consecutive_failures,
            )
        };

        let has_unsent_changes = if self.cloudsync_enabled && network_initialized {
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
            last_sync,
            last_sync_at_ms,
            has_unsent_changes,
            last_error,
            last_error_kind,
            consecutive_failures,
        })
    }

    pub async fn cloudsync_trigger_sync(
        &self,
    ) -> Result<CloudsyncNetworkResult, CloudsyncRuntimeError> {
        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_error = None;
            return Ok(CloudsyncNetworkResult::default());
        }

        let (wait_ms, max_retries) = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            let config = runtime
                .config
                .as_ref()
                .ok_or(CloudsyncRuntimeError::NotConfigured)?;
            (config.wait_ms, config.max_retries)
        };

        if !self.cloudsync_runtime.lock().unwrap().network_initialized {
            return Err(CloudsyncRuntimeError::NotStarted);
        }

        match self.cloudsync_network_sync(wait_ms, max_retries).await {
            Ok(result) => {
                record_sync_result(&self.cloudsync_runtime, result.clone());
                Ok(result)
            }
            Err(error) => {
                record_sync_error(&self.cloudsync_runtime, &error);
                Err(error.into())
            }
        }
    }

    async fn stop_cloudsync_task(&self) -> bool {
        let (task, network_initialized) = {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.running = false;
            (runtime.task.take(), runtime.network_initialized)
        };

        if let Some(mut task) = task {
            if let Some(shutdown_tx) = task.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            let _ = task.join_handle.await;
        }

        network_initialized
    }

    async fn cleanup_failed_cloudsync_start(&self, cleanup_network: bool) {
        if cleanup_network && let Err(error) = self.cloudsync_network_cleanup().await {
            tracing::warn!(%error, "cloudsync cleanup after failed startup failed");
        }
        if self.has_cloudsync()
            && let Err(error) = self.cloudsync_terminate().await
        {
            tracing::warn!(%error, "cloudsync termination after failed startup failed");
        }
        self.cloudsync_connection.lock().await.take();

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.running = false;
        runtime.network_initialized = false;
        runtime.task = None;
    }
}

fn record_sync_result(runtime: &Mutex<CloudsyncRuntimeState>, result: CloudsyncNetworkResult) {
    let mut runtime = runtime.lock().unwrap();
    runtime.last_sync = Some(result);

    if let Some(error) = runtime.last_sync.as_ref().and_then(embedded_sync_error) {
        runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
        runtime.last_error = Some(error);
        runtime.last_error_kind = Some(hypr_cloudsync::ErrorKind::Fatal);
        return;
    }

    runtime.last_sync_at_ms = Some(now_ms());
    runtime.last_error = None;
    runtime.last_error_kind = None;
    runtime.consecutive_failures = 0;
}

fn embedded_sync_error(result: &CloudsyncNetworkResult) -> Option<String> {
    let mut errors = Vec::new();

    if let Some(send) = &result.send {
        if !send.status.eq_ignore_ascii_case("synced")
            && !send.status.eq_ignore_ascii_case("syncing")
        {
            errors.push(format!("send status: {}", send.status));
        }
        if let Some(last_failure) = &send.last_failure {
            errors.push(format!("send failure: {last_failure}"));
        }
    }

    if let Some(receive) = &result.receive {
        if let Some(error) = &receive.error {
            errors.push(format!("receive error: {error}"));
        }
        if let Some(last_failure) = &receive.last_failure {
            errors.push(format!("receive failure: {last_failure}"));
        }
    }

    (!errors.is_empty()).then(|| errors.join("; "))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    fn test_cloudsync_config() -> CloudsyncRuntimeConfig {
        CloudsyncRuntimeConfig {
            connection_string: "sqlitecloud://demo.invalid/app.db?apikey=demo".to_string(),
            auth: super::super::CloudsyncAuth::None,
            tables: Vec::new(),
            sync_interval_ms: 30_000,
            wait_ms: Some(500),
            max_retries: Some(1),
        }
    }

    #[test]
    fn embedded_sync_failures_update_runtime_error_state() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        let result = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "failed".to_string(),
                local_version: 4,
                server_version: 3,
                last_failure: None,
            }),
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 0,
                tables: Vec::new(),
                error: Some("schema mismatch".to_string()),
                last_failure: None,
            }),
        };

        record_sync_result(&runtime, result);

        let runtime = runtime.lock().unwrap();
        assert!(runtime.last_sync.is_some());
        assert!(runtime.last_sync_at_ms.is_none());
        assert_eq!(runtime.consecutive_failures, 1);
        assert_eq!(
            runtime.last_error_kind,
            Some(hypr_cloudsync::ErrorKind::Fatal)
        );
        assert!(
            runtime
                .last_error
                .as_deref()
                .unwrap()
                .contains("schema mismatch")
        );
    }

    #[test]
    fn embedded_sync_in_progress_does_not_update_runtime_error_state() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        let result = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "syncing".to_string(),
                local_version: 4,
                server_version: 3,
                last_failure: None,
            }),
            receive: None,
        };

        record_sync_result(&runtime, result);

        let runtime = runtime.lock().unwrap();
        assert!(runtime.last_sync_at_ms.is_some());
        assert!(runtime.last_error.is_none());
        assert_eq!(runtime.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn logout_releases_connection_after_partial_startup() {
        let mut db = Db::connect_memory_plain().await.unwrap();
        db.cloudsync_enabled = true;
        db.cloudsync_configure(test_cloudsync_config()).unwrap();
        *db.cloudsync_connection.lock().await = Some(db.pool.acquire().await.unwrap());

        db.cloudsync_logout(false).await.unwrap();

        assert!(db.cloudsync_connection.lock().await.is_none());
        assert!(db.cloudsync_runtime.lock().unwrap().config.is_none());
    }

    #[tokio::test]
    async fn restart_after_fatal_exit_cleans_native_state() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        db.cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "managed-database-id".to_string(),
            auth: super::super::CloudsyncAuth::None,
            tables: Vec::new(),
            sync_interval_ms: 30_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .unwrap();
        db.cloudsync_start().await.unwrap();
        {
            let mut connection = db.cloudsync_connection.lock().await;
            sqlx::query("CREATE TEMP TABLE stale_cloudsync_connection (id INTEGER)")
                .execute(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }

        let mut running_task = db.cloudsync_runtime.lock().unwrap().task.take().unwrap();
        let _ = running_task.shutdown_tx.take().unwrap().send(());
        let _ = running_task.join_handle.await;

        let (stale_shutdown_tx, stale_shutdown_rx) = oneshot::channel::<()>();
        let (finished_tx, finished_rx) = oneshot::channel();
        let join_handle = tokio::spawn(async move {
            drop(stale_shutdown_rx);
            let _ = finished_tx.send(());
        });
        finished_rx.await.unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = false;
            runtime.last_error = Some("fatal sync failure".to_string());
            runtime.last_error_kind = Some(hypr_cloudsync::ErrorKind::Fatal);
            runtime.task = Some(CloudsyncBackgroundTask {
                shutdown_tx: Some(stale_shutdown_tx),
                join_handle,
            });
        }

        db.cloudsync_start().await.unwrap();

        {
            let runtime = db.cloudsync_runtime.lock().unwrap();
            assert!(runtime.running);
            assert!(runtime.network_initialized);
            assert!(runtime.task.is_some());
            assert!(runtime.last_error.is_none());
        }
        let marker_count: i64 = {
            let mut connection = db.cloudsync_connection.lock().await;
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'stale_cloudsync_connection'",
            )
            .fetch_one(&mut **connection.as_mut().unwrap())
            .await
            .unwrap()
        };
        assert_eq!(marker_count, 0);
        db.cloudsync_stop().await.unwrap();
    }
}

fn record_sync_error(runtime: &Mutex<CloudsyncRuntimeState>, error: &hypr_cloudsync::Error) {
    let mut runtime = runtime.lock().unwrap();
    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
    runtime.last_error = Some(error.to_string());
    runtime.last_error_kind = Some(error.kind());
}
const MAX_BACKOFF_SECS: u64 = 300;

async fn cloudsync_background_loop(
    pool: SqlitePool,
    connection: Arc<tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>>,
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

                let result = (|| {
                    let connection = Arc::clone(&connection);
                    let pool = pool.clone();
                    async move {
                        sync_cloudsync_connection(&pool, &connection, wait_ms, max_retries).await
                    }
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
                    Ok(result) => {
                        record_sync_result(&runtime_state, result);
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

async fn sync_cloudsync_connection(
    pool: &SqlitePool,
    connection: &tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<CloudsyncNetworkResult, hypr_cloudsync::Error> {
    let mut connection = connection.lock().await;
    if connection.is_none() {
        *connection = Some(pool.acquire().await?);
    }
    let result =
        hypr_cloudsync::network_sync(&mut **connection.as_mut().unwrap(), wait_ms, max_retries)
            .await;
    if pool.options().get_max_connections() == 1 {
        connection.take();
    }
    result
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
