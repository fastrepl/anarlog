use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{ExponentialBuilder, Retryable};
use sqlx::SqlitePool;
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
                table.init_flags,
            )
            .await?;
        }

        self.cloudsync_network_init(&config.connection_string)
            .await?;
        self.apply_cloudsync_auth(&config.auth).await?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let pool = self.pool.clone();
        let runtime_state = Arc::clone(&self.cloudsync_runtime);
        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let wait_ms = config.wait_ms;
        let max_retries = config.max_retries;
        let sync_interval_ms = config.sync_interval_ms;
        let join_handle = tokio::spawn(async move {
            cloudsync_background_loop(
                pool,
                runtime_state,
                sync_lock,
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
        let should_cleanup = self.stop_cloudsync_task().await;

        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.network_initialized = false;
            runtime.last_error = None;
            return Ok(());
        }

        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let _sync_guard = sync_lock.lock().await;
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

    pub async fn cloudsync_logout(
        &self,
        discard_unsent_changes: bool,
    ) -> Result<(), CloudsyncRuntimeError> {
        let network_initialized = self.cloudsync_runtime.lock().unwrap().network_initialized;

        if !self.cloudsync_enabled || !network_initialized {
            self.cloudsync_runtime.lock().unwrap().config = None;
            return Ok(());
        }

        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let _sync_guard = sync_lock.lock().await;
        let has_unsent_changes = self.cloudsync_network_has_unsent_changes().await?;
        if has_unsent_changes && !discard_unsent_changes {
            return Err(CloudsyncRuntimeError::UnsentChanges);
        }

        self.stop_cloudsync_task().await;
        self.cloudsync_network_logout().await?;
        self.cloudsync_network_cleanup().await?;
        if self.has_cloudsync() {
            self.cloudsync_terminate().await?;
        }

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.config = None;
        runtime.network_initialized = false;
        runtime.last_sync = None;
        runtime.last_sync_at_ms = None;
        runtime.last_error = None;
        runtime.last_error_kind = None;
        runtime.consecutive_failures = 0;
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

        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let _sync_guard = sync_lock.lock().await;
        if !self.cloudsync_runtime.lock().unwrap().network_initialized {
            return Err(CloudsyncRuntimeError::NotStarted);
        }

        match hypr_cloudsync::network_sync(&self.pool, wait_ms, max_retries).await {
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
}

fn record_sync_result(runtime: &Mutex<CloudsyncRuntimeState>, result: CloudsyncNetworkResult) {
    let mut runtime = runtime.lock().unwrap();
    runtime.last_sync = Some(result);
    runtime.last_sync_at_ms = Some(now_ms());
    runtime.last_error = None;
    runtime.last_error_kind = None;
    runtime.consecutive_failures = 0;
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
    runtime_state: Arc<Mutex<CloudsyncRuntimeState>>,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
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
                    let pool = pool.clone();
                    let sync_lock = Arc::clone(&sync_lock);
                    async move {
                        let _guard = sync_lock.lock().await;
                        hypr_cloudsync::network_sync(&pool, wait_ms, max_retries).await
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
