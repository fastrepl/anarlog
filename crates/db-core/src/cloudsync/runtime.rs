use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{BackoffBuilder, ExponentialBuilder};
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
    pub async fn cloudsync_configure(
        &self,
        config: CloudsyncRuntimeConfig,
    ) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.cloudsync_configure_locked(config)
    }

    fn cloudsync_configure_locked(
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
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        let was_running = self.cloudsync_runtime.lock().unwrap().running;

        if was_running {
            self.cloudsync_stop_locked().await?;
        }

        self.cloudsync_configure_locked(config)?;

        if was_running {
            self.cloudsync_start_locked().await?;
        }

        Ok(())
    }

    pub async fn cloudsync_start(&self) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.cloudsync_start_locked().await
    }

    async fn cloudsync_start_locked(&self) -> Result<(), CloudsyncRuntimeError> {
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

        if let Err(error) = self.cloudsync_init_enabled_tables(&config.tables).await {
            self.cleanup_failed_cloudsync_start(false).await;
            return Err(error.into());
        }

        if let Err(error) = self.cloudsync_network_init(&config.connection_string).await {
            self.cleanup_failed_cloudsync_start(true).await;
            return Err(error.into());
        }
        if let Err(error) = authenticate_cloudsync_network(
            || self.apply_cloudsync_auth(&config.auth),
            || self.cloudsync_network_cleanup(),
        )
        .await
        {
            self.cleanup_failed_cloudsync_start(true).await;
            return Err(error.into());
        }
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let pool = self.pool.clone();
        let connection = Arc::clone(&self.cloudsync_connection);
        let runtime_state = Arc::clone(&self.cloudsync_runtime);
        let sync_hook = Arc::clone(&self.cloudsync_sync_hook);
        let sync_config = CloudsyncLoopConfig {
            interval: Duration::from_millis(config.sync_interval_ms),
            wait_ms: config.wait_ms,
            max_retries: config.max_retries,
        };
        let join_handle = tokio::spawn(async move {
            cloudsync_background_loop(
                pool,
                connection,
                runtime_state,
                sync_hook,
                sync_config,
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
        let mut first_error = None;

        if self.cloudsync_enabled
            && should_cleanup
            && let Err(error) = self.cloudsync_network_cleanup().await
        {
            first_error = Some(CloudsyncRuntimeError::from(error));
        }

        if self.cloudsync_enabled
            && self.has_cloudsync()
            && let Err(error) = self.cloudsync_terminate_and_close().await
            && first_error.is_none()
        {
            first_error = Some(CloudsyncRuntimeError::from(error));
        }

        if let Err(error) = self.cloudsync_close_connection().await
            && first_error.is_none()
        {
            first_error = Some(CloudsyncRuntimeError::from(error));
        }

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = false;
        runtime.last_error = None;
        first_error.map_or(Ok(()), Err)
    }

    pub async fn cloudsync_suspend(&self) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        let stop_result = self.cloudsync_stop_locked().await;

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.config = None;
        runtime.last_sync = None;
        runtime.last_sync_at_ms = None;
        runtime.last_error = None;
        runtime.last_error_kind = None;
        runtime.consecutive_failures = 0;
        stop_result
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
            self.cloudsync_terminate_and_close().await
        } else {
            Ok(())
        };
        let close_result = self.cloudsync_close_connection().await;

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
        close_result?;
        Ok(())
    }

    pub async fn cloudsync_status(&self) -> Result<CloudsyncStatus, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
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
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
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

        let result = async {
            run_before_sync_hook(&self.cloudsync_sync_hook, &self.pool).await?;
            let result = self.cloudsync_network_sync(wait_ms, max_retries).await?;
            run_after_sync_hook(&self.cloudsync_sync_hook, &self.pool).await?;
            Ok::<_, hypr_cloudsync::Error>(result)
        }
        .await;

        match result {
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
            && let Err(error) = self.cloudsync_terminate_and_close().await
        {
            tracing::warn!(%error, "cloudsync teardown after failed startup failed");
        }
        if let Err(error) = self.cloudsync_close_connection().await {
            tracing::warn!(%error, "cloudsync connection close after failed startup failed");
        }

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.running = false;
        runtime.network_initialized = false;
        runtime.task = None;
    }
}

async fn authenticate_cloudsync_network<A, AF, C, CF>(
    authenticate: A,
    cleanup: C,
) -> Result<(), hypr_cloudsync::Error>
where
    A: FnOnce() -> AF,
    AF: Future<Output = Result<(), hypr_cloudsync::Error>>,
    C: FnOnce() -> CF,
    CF: Future<Output = Result<(), hypr_cloudsync::Error>>,
{
    if let Err(auth_error) = authenticate().await {
        if let Err(cleanup_error) = cleanup().await {
            tracing::warn!(
                error = %cleanup_error,
                "failed to clean up cloudsync network after authentication failure",
            );
        }
        return Err(auth_error);
    }

    Ok(())
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

    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::{CloudsyncAuth, CloudsyncTableSpec, DbOpenOptions, DbStorage};
    #[test]
    fn embedded_sync_failures_update_runtime_error_state() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        let result = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "failed".to_string(),
                local_version: 4,
                server_version: 3,
                chunks: 1,
                bytes: 1024,
                last_failure: None,
            }),
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 0,
                tables: Vec::new(),
                chunks: 0,
                bytes: 0,
                complete: true,
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
                chunks: 1,
                bytes: 1024,
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
        db.cloudsync_configure(test_cloudsync_config())
            .await
            .unwrap();
        *db.cloudsync_connection.lock().await = Some(db.pool.acquire().await.unwrap());

        db.cloudsync_logout(false).await.unwrap();

        assert!(db.cloudsync_connection.lock().await.is_none());
        assert!(db.cloudsync_runtime.lock().unwrap().config.is_none());
    }

    #[tokio::test]
    async fn authentication_failure_cleans_up_initialized_network() {
        let cleanup_called = AtomicBool::new(false);

        let error = authenticate_cloudsync_network(
            || async {
                Err::<(), _>(hypr_cloudsync::Error::from(std::io::Error::other(
                    "authentication rejected",
                )))
            },
            || async {
                cleanup_called.store(true, Ordering::SeqCst);
                Ok::<(), hypr_cloudsync::Error>(())
            },
        )
        .await
        .unwrap_err();

        assert!(cleanup_called.load(Ordering::SeqCst));
        assert!(error.to_string().contains("authentication rejected"));
    }

    #[tokio::test]
    async fn configure_start_and_suspend_transitions_are_serialized() {
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        db.cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "managed-database-id".to_string(),
            auth: CloudsyncAuth::None,
            tables: Vec::new(),
            sync_interval_ms: 30_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .await
        .unwrap();

        let lifecycle = db.cloudsync_lifecycle.lock().await;
        let mut configure = Box::pin(db.cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "next-managed-database-id".to_string(),
            auth: CloudsyncAuth::None,
            tables: Vec::new(),
            sync_interval_ms: 45_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        }));
        tokio::select! {
            biased;
            result = &mut configure => panic!("configure bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        let mut start = Box::pin(db.cloudsync_start());
        tokio::select! {
            biased;
            result = &mut start => panic!("start bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        let mut suspend = Box::pin(db.cloudsync_suspend());
        tokio::select! {
            biased;
            result = &mut suspend => panic!("suspend bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        drop(lifecycle);
        configure.await.unwrap();
        start.await.unwrap();
        suspend.await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(!status.configured);
        assert!(!status.running);
        assert!(!status.network_initialized);
    }

    #[tokio::test]
    async fn status_and_manual_sync_wait_for_suspend_teardown() {
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        db.cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "managed-database-id".to_string(),
            auth: CloudsyncAuth::None,
            tables: Vec::new(),
            sync_interval_ms: 30_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .await
        .unwrap();

        let lifecycle = db.cloudsync_lifecycle.lock().await;
        let mut suspend = Box::pin(db.cloudsync_suspend());
        tokio::select! {
            biased;
            result = &mut suspend => panic!("suspend bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        let mut status = Box::pin(db.cloudsync_status());
        tokio::select! {
            biased;
            result = &mut status => panic!("status bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        let mut trigger = Box::pin(db.cloudsync_trigger_sync());
        tokio::select! {
            biased;
            result = &mut trigger => panic!("manual sync bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        drop(lifecycle);
        suspend.await.unwrap();
        assert!(!status.await.unwrap().configured);
        assert_eq!(trigger.await.unwrap(), CloudsyncNetworkResult::default());
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
        .await
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

    #[tokio::test]
    async fn suspend_interrupts_active_retry_backoff() {
        let db = Db::connect_memory_plain().await.unwrap();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let (retry_started_tx, retry_started_rx) = oneshot::channel();
        let join_handle = tokio::spawn(async move {
            let _ = retry_started_tx.send(());
            assert!(!wait_for_retry_or_shutdown(Duration::from_secs(60), &mut shutdown_rx).await);
        });
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = true;
            runtime.task = Some(CloudsyncBackgroundTask {
                shutdown_tx: Some(shutdown_tx),
                join_handle,
            });
        }
        retry_started_rx.await.unwrap();

        tokio::time::timeout(Duration::from_secs(1), db.cloudsync_suspend())
            .await
            .expect("suspend waited for retry backoff")
            .unwrap();

        assert!(!db.cloudsync_status().await.unwrap().running);
    }

    #[tokio::test]
    async fn suspend_stops_runtime_and_clears_in_memory_credentials() {
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Memory,
            cloudsync_enabled: false,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        db.cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "managed-database-id".to_string(),
            auth: CloudsyncAuth::Token {
                token: "secret-token".to_string(),
            },
            tables: vec![CloudsyncTableSpec {
                table_name: "sessions".to_string(),
                crdt_algo: None,
                init_flags: None,
                enabled: true,
            }],
            sync_interval_ms: 30_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .await
        .unwrap();

        db.cloudsync_start().await.unwrap();
        db.cloudsync_suspend().await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(!status.configured);
        assert!(!status.running);
        assert!(!status.network_initialized);
    }

    #[tokio::test]
    async fn suspend_clears_runtime_state_when_native_teardown_fails() {
        let db = Db::connect_memory().await.unwrap();
        db.cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "managed-database-id".to_string(),
            auth: CloudsyncAuth::Token {
                token: "secret-token".to_string(),
            },
            tables: Vec::new(),
            sync_interval_ms: 30_000,
            wait_ms: Some(5_000),
            max_retries: Some(3),
        })
        .await
        .unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = true;
            runtime.network_initialized = true;
        }
        db.pool().close().await;

        db.cloudsync_suspend().await.unwrap_err();

        let status = db.cloudsync_status().await.unwrap();
        assert!(!status.configured);
        assert!(!status.running);
        assert!(!status.network_initialized);
        assert!(db.cloudsync_connection.lock().await.is_none());
    }
}

fn record_sync_error(runtime: &Mutex<CloudsyncRuntimeState>, error: &hypr_cloudsync::Error) {
    let mut runtime = runtime.lock().unwrap();
    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
    runtime.last_error = Some(error.to_string());
    runtime.last_error_kind = Some(error.kind());
}
const MAX_BACKOFF_SECS: u64 = 300;

#[derive(Clone, Copy)]
struct CloudsyncLoopConfig {
    interval: Duration,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
}

async fn cloudsync_background_loop(
    pool: SqlitePool,
    connection: Arc<tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>>,
    runtime_state: Arc<Mutex<CloudsyncRuntimeState>>,
    sync_hook: Arc<Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>>,
    config: CloudsyncLoopConfig,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            _ = tokio::time::sleep(config.interval) => {
                let Some(result) = sync_cloudsync_with_retry(
                    &pool,
                    &connection,
                    &runtime_state,
                    &sync_hook,
                    config,
                    &mut shutdown_rx,
                )
                .await else {
                    break;
                };

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

async fn sync_cloudsync_with_retry(
    pool: &SqlitePool,
    connection: &tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>,
    runtime_state: &Mutex<CloudsyncRuntimeState>,
    sync_hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
    config: CloudsyncLoopConfig,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<Result<CloudsyncNetworkResult, hypr_cloudsync::Error>> {
    let mut backoff = ExponentialBuilder::default()
        .with_min_delay(config.interval)
        .with_max_delay(Duration::from_secs(MAX_BACKOFF_SECS))
        .with_jitter()
        .build();

    loop {
        match sync_cloudsync_connection(
            pool,
            connection,
            sync_hook,
            config.wait_ms,
            config.max_retries,
        )
        .await
        {
            Err(error) if error.kind() == hypr_cloudsync::ErrorKind::Transient => {
                let Some(retry_after) = backoff.next() else {
                    return Some(Err(error));
                };

                let failures = {
                    let mut runtime = runtime_state.lock().unwrap();
                    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                    runtime.last_error = Some(error.to_string());
                    runtime.last_error_kind = Some(error.kind());
                    runtime.consecutive_failures
                };
                tracing::warn!(
                    error = %error,
                    retry_after = ?retry_after,
                    failures,
                    "cloudsync transient error, retrying",
                );

                if !wait_for_retry_or_shutdown(retry_after, shutdown_rx).await {
                    return None;
                }
            }
            result => return Some(result),
        }
    }
}

async fn wait_for_retry_or_shutdown(
    retry_after: Duration,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> bool {
    tokio::select! {
        _ = &mut *shutdown_rx => false,
        _ = tokio::time::sleep(retry_after) => true,
    }
}

async fn sync_cloudsync_connection(
    pool: &SqlitePool,
    connection: &tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>,
    sync_hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<CloudsyncNetworkResult, hypr_cloudsync::Error> {
    run_before_sync_hook(sync_hook, pool).await?;
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
    let result = result?;
    run_after_sync_hook(sync_hook, pool).await?;
    Ok(result)
}

async fn run_before_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
    pool: &SqlitePool,
) -> Result<(), hypr_cloudsync::Error> {
    let hook = hook.lock().unwrap().clone();
    match hook {
        Some(hook) => hook.before_sync(pool).await,
        None => Ok(()),
    }
}

async fn run_after_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
    pool: &SqlitePool,
) -> Result<(), hypr_cloudsync::Error> {
    let hook = hook.lock().unwrap().clone();
    match hook {
        Some(hook) => hook.after_sync(pool).await,
        None => Ok(()),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
