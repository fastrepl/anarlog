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
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
        self.cloudsync_configure_locked(config)
    }

    fn cloudsync_configure_locked(
        &self,
        config: CloudsyncRuntimeConfig,
    ) -> Result<(), CloudsyncRuntimeError> {
        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        if runtime.running || runtime.network_initialized || runtime.task.is_some() {
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
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
        let (was_running, had_transport) = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            (
                runtime.running,
                runtime.network_initialized || runtime.task.is_some(),
            )
        };

        if had_transport {
            self.cloudsync_stop_locked().await?;
        }

        self.cloudsync_configure_locked(config)?;

        if was_running {
            self.cloudsync_start_locked().await?;
        }

        Ok(())
    }

    pub async fn cloudsync_start(&self) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
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
            runtime.outbound_work_state = None;
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

        self.initialize_cloudsync_transport(&config).await?;
        {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_error = None;
            runtime.last_error_kind = None;
            runtime.consecutive_failures = 0;
            runtime.outbound_work_state = None;
        }
        self.start_cloudsync_background_task(&config);

        Ok(())
    }

    pub async fn cloudsync_prepare_manual_transport(
        &self,
        config: CloudsyncRuntimeConfig,
    ) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
        if !self.cloudsync_enabled {
            return Err(CloudsyncRuntimeError::Unavailable);
        }

        {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            if runtime.running {
                return Err(CloudsyncRuntimeError::RestartRequired);
            }
        }

        let needs_cleanup = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.network_initialized || runtime.task.is_some()
        };
        if needs_cleanup {
            self.cloudsync_stop_locked().await?;
        }

        let config = config.normalized()?;
        {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(config.clone());
            runtime.last_error = None;
            runtime.last_error_kind = None;
            runtime.consecutive_failures = 0;
            runtime.outbound_work_state = None;
        }

        self.initialize_cloudsync_transport(&config).await?;
        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.running = false;
        runtime.network_initialized = true;
        runtime.outbound_work_state = None;
        runtime.last_error = None;
        runtime.last_error_kind = None;
        runtime.consecutive_failures = 0;
        Ok(())
    }

    pub async fn cloudsync_resume_prepared_transport(&self) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
        if !self.cloudsync_enabled {
            return Err(CloudsyncRuntimeError::Unavailable);
        }

        let (config, finished_task) = {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            if runtime.running {
                return Ok(());
            }
            if !runtime.network_initialized {
                return Err(CloudsyncRuntimeError::NotStarted);
            }
            let finished_task = match runtime.task.as_ref() {
                Some(task) if task.join_handle.is_finished() => runtime.task.take(),
                Some(_) => return Err(CloudsyncRuntimeError::RestartRequired),
                None => None,
            };
            let config = runtime
                .config
                .clone()
                .ok_or(CloudsyncRuntimeError::NotConfigured)?;
            (config, finished_task)
        };

        if let Some(mut task) = finished_task {
            task.shutdown_tx.take();
            let _ = task.join_handle.await;
        }

        {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_error = None;
            runtime.last_error_kind = None;
            runtime.consecutive_failures = 0;
            runtime.outbound_work_state = None;
        }
        self.start_cloudsync_background_task(&config);
        Ok(())
    }

    async fn initialize_cloudsync_transport(
        &self,
        config: &CloudsyncRuntimeConfig,
    ) -> Result<(), CloudsyncRuntimeError> {
        if let Err(error) = self.cloudsync_init_enabled_tables(&config.tables).await {
            self.cleanup_failed_cloudsync_start(false).await;
            return Err(error);
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

        Ok(())
    }

    fn start_cloudsync_background_task(&self, config: &CloudsyncRuntimeConfig) {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let pool = self.pool.clone();
        let connection = Arc::clone(&self.cloudsync_connection);
        let interrupt = Arc::clone(&self.cloudsync_interrupt);
        let sync_operation = Arc::clone(&self.cloudsync_sync_operation);
        let sync_requested = Arc::clone(&self.cloudsync_sync_requested);
        let runtime_state = Arc::clone(&self.cloudsync_runtime);
        let sync_hook = Arc::clone(&self.cloudsync_sync_hook);
        let context = CloudsyncLoopContext {
            pool,
            connection,
            interrupt,
            sync_operation,
            sync_requested,
            runtime_state,
            sync_hook,
            config: CloudsyncLoopConfig {
                interval: Duration::from_millis(config.sync_interval_ms),
            },
        };
        let join_handle = tokio::spawn(async move {
            cloudsync_background_loop(context, shutdown_rx).await;
        });

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.network_initialized = true;
        runtime.task = Some(CloudsyncBackgroundTask {
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
    }

    async fn lock_cloudsync_lifecycle_cancelling_active_sync(
        &self,
    ) -> tokio::sync::MutexGuard<'_, ()> {
        let lifecycle = self.cloudsync_lifecycle.lock();
        tokio::pin!(lifecycle);
        let mut cancellation_interval = tokio::time::interval(Duration::from_millis(25));
        loop {
            tokio::select! {
                biased;
                guard = &mut lifecycle => return guard,
                _ = cancellation_interval.tick() => {
                    cancel_active_sync_hook(&self.cloudsync_sync_hook);
                    self.cloudsync_interrupt_sync();
                }
            }
        }
    }

    pub async fn cloudsync_stop(&self) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
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
            first_error = Some(error);
        }

        if let Err(error) = self.cloudsync_close_connection().await
            && first_error.is_none()
        {
            first_error = Some(CloudsyncRuntimeError::from(error));
        }

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.network_initialized = false;
        runtime.outbound_work_state = None;
        runtime.last_error = None;
        first_error.map_or(Ok(()), Err)
    }

    pub async fn cloudsync_suspend(&self) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;
        let stop_result = self.cloudsync_stop_locked().await;

        let mut runtime = self.cloudsync_runtime.lock().unwrap();
        runtime.config = None;
        runtime.last_sync = None;
        runtime.last_sync_at_ms = None;
        runtime.outbound_work_state = None;
        runtime.last_error = None;
        runtime.last_error_kind = None;
        runtime.consecutive_failures = 0;
        stop_result
    }

    pub async fn cloudsync_logout(
        &self,
        discard_unsent_changes: bool,
    ) -> Result<(), CloudsyncRuntimeError> {
        let _lifecycle = self.lock_cloudsync_lifecycle_cancelling_active_sync().await;

        if !self.cloudsync_enabled {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.config = None;
            runtime.outbound_work_state = None;
            return Ok(());
        }

        let resume_config = {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            if runtime.running {
                Some(
                    runtime
                        .config
                        .clone()
                        .ok_or(CloudsyncRuntimeError::NotConfigured)?,
                )
            } else {
                None
            }
        };
        let network_initialized = self.stop_cloudsync_task().await;
        let sync_operation = self.cloudsync_sync_operation.lock().await;
        let has_unsent_changes = if network_initialized && !discard_unsent_changes {
            match self.try_cloudsync_has_local_unsent_changes().await {
                Ok(Some(has_unsent_changes)) => has_unsent_changes,
                Ok(None) => {
                    drop(sync_operation);
                    if let Some(config) = resume_config.as_ref() {
                        self.start_cloudsync_background_task(config);
                    }
                    return Err(CloudsyncRuntimeError::LocalStatusBusy);
                }
                Err(error) => {
                    drop(sync_operation);
                    if let Some(config) = resume_config.as_ref() {
                        self.start_cloudsync_background_task(config);
                    }
                    return Err(error.into());
                }
            }
        } else {
            false
        };
        if has_unsent_changes && !discard_unsent_changes {
            drop(sync_operation);
            if let Some(config) = resume_config.as_ref() {
                self.start_cloudsync_background_task(config);
            }
            return Err(CloudsyncRuntimeError::UnsentChanges);
        }

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
        runtime.outbound_work_state = None;
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
        let lifecycle = self.cloudsync_lifecycle.try_lock().ok();
        let sync_operation = self.cloudsync_sync_operation.try_lock().ok();
        let activity_paused = cloudsync_activity_paused(&self.cloudsync_sync_hook);
        let (
            config,
            running,
            network_initialized,
            last_sync,
            last_sync_at_ms,
            outbound_work_state,
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
                runtime.outbound_work_state,
                runtime.last_error.clone(),
                runtime.last_error_kind.map(CloudsyncErrorKind::from),
                runtime.consecutive_failures,
            )
        };

        let has_unsent_changes = if activity_paused {
            None
        } else if self.cloudsync_enabled
            && network_initialized
            && running
            && lifecycle.is_some()
            && sync_operation.is_some()
        {
            self.try_cloudsync_has_local_unsent_changes().await?
        } else if self.cloudsync_enabled
            && network_initialized
            && running
            && lifecycle.is_some()
            && sync_operation.is_none()
        {
            outbound_work_state
        } else {
            None
        };

        Ok(CloudsyncStatus {
            cloudsync_enabled: self.cloudsync_enabled,
            extension_loaded: self.has_cloudsync(),
            configured: config.is_some(),
            running,
            network_initialized,
            activity_paused,
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

        {
            let runtime = self.cloudsync_runtime.lock().unwrap();
            runtime
                .config
                .as_ref()
                .ok_or(CloudsyncRuntimeError::NotConfigured)?;
        }

        if !self.cloudsync_runtime.lock().unwrap().network_initialized {
            return Err(CloudsyncRuntimeError::NotStarted);
        }

        let result = sync_cloudsync_connection(
            &self.pool,
            &self.cloudsync_connection,
            &self.cloudsync_interrupt,
            &self.cloudsync_sync_operation,
            &self.cloudsync_runtime,
            &self.cloudsync_sync_hook,
        )
        .await;

        match result {
            Ok(CloudsyncStepOutcome::Completed(step)) => {
                record_sync_result(
                    &self.cloudsync_runtime,
                    step.network.clone(),
                    step.local_work_remaining,
                );
                Ok(step.network)
            }
            Ok(CloudsyncStepOutcome::Deferred) => Ok(CloudsyncNetworkResult::default()),
            Err(error) => {
                record_sync_error(&self.cloudsync_runtime, &error);
                Err(error.into())
            }
        }
    }

    pub async fn cloudsync_wait_for_sync_idle(&self) {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.cloudsync_connection.lock().await;
        let Some(connection) = connection.as_mut() else {
            return;
        };
        match connection.lock_handle().await {
            Ok(worker_idle) => drop(worker_idle),
            Err(error) => {
                tracing::warn!(%error, "failed to fence the CloudSync SQLite worker");
            }
        }
    }

    pub fn cloudsync_interrupt_sync(&self) -> bool {
        self.cloudsync_interrupt.interrupt()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn cloudsync_interrupt_registered(&self) -> bool {
        self.cloudsync_interrupt.is_registered()
    }

    pub fn cloudsync_request_sync(&self) {
        if self.cloudsync_runtime.lock().unwrap().running {
            self.cloudsync_sync_requested.notify_one();
        }
    }

    pub fn cloudsync_runtime_observation(&self) -> (bool, Option<CloudsyncNetworkResult>) {
        let runtime = self.cloudsync_runtime.lock().unwrap();
        (runtime.running, runtime.last_sync.clone())
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
        runtime.outbound_work_state = None;
    }

    async fn try_cloudsync_has_local_unsent_changes(
        &self,
    ) -> Result<Option<bool>, hypr_cloudsync::Error> {
        let Some(mut connection) = self.pool.try_acquire() else {
            return Ok(None);
        };
        let result = super::ops::cloudsync_has_local_unsent_changes_on(&mut *connection)
            .await
            .map(Some);
        connection.return_to_pool().await;
        result
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

fn record_sync_result(
    runtime: &Mutex<CloudsyncRuntimeState>,
    result: CloudsyncNetworkResult,
    local_work_remaining: bool,
) {
    let mut runtime = runtime.lock().unwrap();
    runtime.last_sync = Some(result);

    if let Some(error) = runtime.last_sync.as_ref().and_then(embedded_sync_error) {
        runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
        runtime.last_error = Some(error);
        runtime.last_error_kind = runtime
            .last_error
            .as_deref()
            .map(|error| hypr_cloudsync::Error::Io(std::io::Error::other(error)).kind());
        return;
    }

    if runtime
        .last_sync
        .as_ref()
        .is_some_and(|result| sync_result_settled(result) && !local_work_remaining)
    {
        runtime.last_sync_at_ms = Some(now_ms());
    }
    runtime.last_error = None;
    runtime.last_error_kind = None;
    runtime.consecutive_failures = 0;
}

fn sync_result_settled(result: &CloudsyncNetworkResult) -> bool {
    sync_send_settled(result)
        && result.receive.as_ref().is_some_and(|receive| {
            receive.complete && receive.error.is_none() && receive.last_failure.is_none()
        })
}

fn sync_send_settled(result: &CloudsyncNetworkResult) -> bool {
    result.send.as_ref().is_none_or(|send| {
        send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none()
    })
}

fn sync_result_needs_receive_progress(result: &CloudsyncNetworkResult) -> bool {
    embedded_sync_error(result).is_none()
        && result.receive.is_some()
        && !sync_result_settled(result)
}

fn sync_result_uploaded(result: &CloudsyncNetworkResult) -> bool {
    result.send.as_ref().is_some_and(|send| {
        send.status.eq_ignore_ascii_case("synced") && send.chunks > 0 && send.last_failure.is_none()
    })
}

fn cloudsync_next_delay(
    result: Option<&CloudsyncNetworkResult>,
    local_work_remaining: bool,
    interval: Duration,
) -> Duration {
    match result {
        None => Duration::ZERO,
        Some(result) if sync_result_needs_receive_progress(result) => CLOUDSYNC_PROGRESS_INTERVAL,
        Some(result) if local_work_remaining && !sync_result_uploaded(result) => {
            CLOUDSYNC_PROGRESS_INTERVAL
        }
        Some(_) => interval,
    }
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

    #[cfg(target_os = "macos")]
    async fn assert_interrupts_stalled_native_request_once() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (accepted_tx, accepted_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            accepted_tx.send(()).unwrap();
            let _stream = stream;
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
        });

        let db = Arc::new(Db::connect_memory().await.unwrap());
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        {
            let mut connection = db.cloudsync_connection.lock().await;
            *connection = Some(db.pool.acquire().await.unwrap());
            sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
                .bind(endpoint)
                .bind("interrupt-test")
                .fetch_optional(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }

        let request_db = Arc::clone(&db);
        let request = tokio::spawn(async move {
            let _sync_operation = request_db.cloudsync_sync_operation.lock().await;
            let mut connection = request_db.cloudsync_connection.lock().await;
            super::super::ops::interruptible_network_receive_changes(
                connection.as_mut().unwrap(),
                &request_db.cloudsync_interrupt,
            )
            .await
        });
        tokio::task::spawn_blocking(move || {
            accepted_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("native CloudSync request did not reach the blackhole server");
        })
        .await
        .unwrap();

        let interrupted_at = std::time::Instant::now();
        while !request.is_finished() {
            db.cloudsync_interrupt_sync();
            assert!(
                interrupted_at.elapsed() < Duration::from_secs(2),
                "native CloudSync request did not honor sqlite3_interrupt"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let result = request.await.unwrap();
        assert!(result.is_err(), "interrupted native request succeeded");
        assert!(interrupted_at.elapsed() < Duration::from_secs(2));
        tokio::time::timeout(
            Duration::from_millis(100),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect("sync operation released before its SQLite worker became idle");

        {
            let mut connection = db.cloudsync_connection.lock().await;
            let value: i64 = sqlx::query_scalar("SELECT 1")
                .fetch_one(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
            assert_eq!(value, 1);
            let worker_idle = connection.as_mut().unwrap().lock_handle().await.unwrap();
            drop(worker_idle);
        }

        let _ = release_tx.send(());
        server.join().unwrap();
        db.cloudsync_close_connection().await.unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn native_cloudsync_interrupt_drains_blackhole_http_and_reuses_connection() {
        for _ in 0..3 {
            assert_interrupts_stalled_native_request_once().await;
        }
    }

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

    async fn db_with_local_unsent_changes() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
            table_name: "items".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }])
        .await
        .unwrap();
        let mut connection = db.pool().acquire().await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'pending')")
            .execute(&mut *connection)
            .await
            .unwrap();
        connection.return_to_pool().await;
        assert!(
            db.pool().num_idle() > 0,
            "cloudsync test pool has no idle connections (size={}, max={})",
            db.pool().size(),
            db.pool().options().get_max_connections(),
        );
        (dir, db)
    }

    use std::future::pending;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use crate::{CloudsyncAuth, CloudsyncTableSpec, DbOpenOptions, DbStorage};
    #[test]
    fn embedded_sync_failures_update_runtime_error_state() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        runtime.lock().unwrap().last_sync_at_ms = Some(42);
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

        record_sync_result(&runtime, result, false);

        let runtime = runtime.lock().unwrap();
        assert!(runtime.last_sync.is_some());
        assert_eq!(runtime.last_sync_at_ms, Some(42));
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
    fn embedded_sqlite_contention_remains_retryable() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        let result = CloudsyncNetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 0,
                tables: Vec::new(),
                chunks: 0,
                bytes: 0,
                complete: false,
                error: Some("database is locked".to_string()),
                last_failure: None,
            }),
        };

        record_sync_result(&runtime, result, false);

        let runtime = runtime.lock().unwrap();
        assert_eq!(
            runtime.last_error_kind,
            Some(hypr_cloudsync::ErrorKind::Transient)
        );
        assert_eq!(runtime.consecutive_failures, 1);
    }

    #[test]
    fn embedded_sync_in_progress_preserves_last_successful_sync() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        runtime.lock().unwrap().last_sync_at_ms = Some(42);
        let result = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "syncing".to_string(),
                local_version: 4,
                server_version: 3,
                chunks: 1,
                bytes: 1024,
                last_failure: None,
            }),
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 3,
                tables: vec!["sessions".to_string()],
                chunks: 1,
                bytes: 2048,
                complete: false,
                error: None,
                last_failure: None,
            }),
        };

        record_sync_result(&runtime, result, false);

        let runtime = runtime.lock().unwrap();
        assert_eq!(runtime.last_sync_at_ms, Some(42));
        assert!(runtime.last_error.is_none());
        assert_eq!(runtime.consecutive_failures, 0);
    }

    #[test]
    fn initial_sync_stays_unsettled_while_receive_is_in_progress() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        let result = CloudsyncNetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 3,
                tables: vec!["sessions".to_string()],
                chunks: 1,
                bytes: 2048,
                complete: false,
                error: None,
                last_failure: None,
            }),
        };

        record_sync_result(&runtime, result, false);

        let runtime = runtime.lock().unwrap();
        assert!(runtime.last_sync_at_ms.is_none());
        assert!(runtime.last_error.is_none());
    }

    #[test]
    fn completed_receive_marks_sync_complete_without_a_send_result() {
        let runtime = Mutex::new(CloudsyncRuntimeState::default());
        let result = CloudsyncNetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 0,
                tables: Vec::new(),
                chunks: 0,
                bytes: 0,
                complete: true,
                error: None,
                last_failure: None,
            }),
        };

        record_sync_result(&runtime, result, false);

        assert!(runtime.lock().unwrap().last_sync_at_ms.is_some());
    }

    #[test]
    fn settled_network_preserves_last_success_while_local_work_remains() {
        let runtime = Mutex::new(CloudsyncRuntimeState {
            last_sync_at_ms: Some(42),
            ..Default::default()
        });
        let result = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 2,
                server_version: 2,
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
                error: None,
                last_failure: None,
            }),
        };

        record_sync_result(&runtime, result, true);

        assert_eq!(runtime.lock().unwrap().last_sync_at_ms, Some(42));
    }

    #[test]
    fn bounded_sync_combines_send_and_receive_results() {
        let send = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 4,
                server_version: 4,
                chunks: 1,
                bytes: 1024,
                last_failure: None,
            }),
            receive: None,
        };
        let receive = CloudsyncNetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 3,
                tables: vec!["sessions".to_string()],
                chunks: 1,
                bytes: 2048,
                complete: false,
                error: None,
                last_failure: None,
            }),
        };

        let result = merge_bounded_sync_results(send.clone(), receive.clone());

        assert_eq!(result.send, send.send);
        assert_eq!(result.receive, receive.receive);
        assert!(sync_result_needs_receive_progress(&result));
    }

    #[test]
    fn background_sync_starts_immediately_and_continues_incomplete_receive_promptly() {
        let interval = Duration::from_secs(30);
        let incomplete = CloudsyncNetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 1,
                tables: vec!["e2ee_records".to_string()],
                chunks: 1,
                bytes: 1024,
                complete: false,
                error: None,
                last_failure: None,
            }),
        };

        assert_eq!(cloudsync_next_delay(None, false, interval), Duration::ZERO);
        assert_eq!(
            cloudsync_next_delay(Some(&incomplete), false, interval),
            CLOUDSYNC_PROGRESS_INTERVAL
        );

        let send_in_progress = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "syncing".to_string(),
                local_version: 3,
                server_version: 2,
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
                error: None,
                last_failure: None,
            }),
        };
        assert_eq!(
            cloudsync_next_delay(Some(&send_in_progress), false, interval),
            CLOUDSYNC_PROGRESS_INTERVAL
        );

        let settled = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 3,
                server_version: 3,
                chunks: 0,
                bytes: 0,
                last_failure: None,
            }),
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 0,
                tables: Vec::new(),
                chunks: 0,
                bytes: 0,
                complete: true,
                error: None,
                last_failure: None,
            }),
        };
        assert_eq!(
            cloudsync_next_delay(Some(&settled), true, interval),
            CLOUDSYNC_PROGRESS_INTERVAL
        );

        let uploaded = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 4,
                server_version: 4,
                chunks: 1,
                bytes: 1024,
                last_failure: None,
            }),
            receive: settled.receive,
        };
        assert_eq!(
            cloudsync_next_delay(Some(&uploaded), true, interval),
            interval
        );
    }

    #[derive(Default)]
    struct RecordingSyncHook {
        directive: crate::CloudsyncSyncDirective,
        local_work_remaining: bool,
        activity_paused: AtomicBool,
        before_calls: AtomicUsize,
        after_result: Mutex<Option<CloudsyncNetworkResult>>,
    }

    impl crate::CloudsyncSyncHook for RecordingSyncHook {
        fn activity_paused(&self) -> bool {
            self.activity_paused.load(Ordering::SeqCst)
        }

        fn before_sync<'a>(
            &'a self,
            _pool: &'a SqlitePool,
        ) -> crate::CloudsyncBeforeHookFuture<'a> {
            Box::pin(async move {
                self.before_calls.fetch_add(1, Ordering::SeqCst);
                Ok(self.directive)
            })
        }

        fn after_sync<'a>(
            &'a self,
            _pool: &'a SqlitePool,
            result: &'a CloudsyncNetworkResult,
        ) -> crate::CloudsyncHookFuture<'a> {
            Box::pin(async move {
                *self.after_result.lock().unwrap() = Some(result.clone());
                Ok(crate::CloudsyncHookOutcome {
                    local_work_remaining: self.local_work_remaining,
                    deferred: false,
                })
            })
        }
    }

    #[tokio::test]
    async fn before_sync_hook_can_select_receive_only_transport() {
        let db = Db::connect_memory_plain().await.unwrap();
        let recording_hook = Arc::new(RecordingSyncHook {
            directive: crate::CloudsyncSyncDirective::ReceiveOnly,
            ..Default::default()
        });
        let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook;
        let hook = Mutex::new(Some(hook));

        assert_eq!(
            run_before_sync_hook(&hook, db.pool()).await.unwrap(),
            crate::CloudsyncSyncDirective::ReceiveOnly
        );
    }

    #[tokio::test]
    async fn deferred_before_sync_hook_never_starts_native_transport() {
        let db = Db::connect_memory().await.unwrap();
        sqlx::query("CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        let recording_hook = Arc::new(RecordingSyncHook {
            directive: crate::CloudsyncSyncDirective::Deferred,
            ..Default::default()
        });
        let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
        let hook = Mutex::new(Some(hook));

        let result = sync_cloudsync_connection(
            db.pool(),
            &db.cloudsync_connection,
            &db.cloudsync_interrupt,
            &db.cloudsync_sync_operation,
            &db.cloudsync_runtime,
            &hook,
        )
        .await
        .unwrap();

        assert!(matches!(result, CloudsyncStepOutcome::Deferred));
        assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 1);
        assert!(recording_hook.after_result.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn existing_native_pending_batch_skips_before_sync_hook() {
        let db = Db::connect_memory().await.unwrap();
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'pending')")
            .execute(db.pool())
            .await
            .unwrap();
        let recording_hook = Arc::new(RecordingSyncHook::default());
        let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
        let hook = Mutex::new(Some(hook));

        let result = sync_cloudsync_connection(
            db.pool(),
            &db.cloudsync_connection,
            &db.cloudsync_interrupt,
            &db.cloudsync_sync_operation,
            &db.cloudsync_runtime,
            &hook,
        )
        .await;
        let Err(error) = result else {
            panic!("sync unexpectedly succeeded");
        };

        assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 0);
        assert!(
            error
                .to_string()
                .contains("Unable to retrieve CloudSync network context")
        );
    }

    #[tokio::test]
    async fn activity_pause_precedes_an_existing_native_pending_batch() {
        let db = Db::connect_memory().await.unwrap();
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'pending')")
            .execute(db.pool())
            .await
            .unwrap();
        let pending_before = {
            let mut connection = db.pool().acquire().await.unwrap();
            crate::cloudsync::ops::ensure_pending_payload_fits(
                &mut connection,
                &db.cloudsync_interrupt,
            )
            .await
            .unwrap()
        };
        assert!(pending_before.chunks > 0);

        let recording_hook = Arc::new(RecordingSyncHook {
            activity_paused: AtomicBool::new(true),
            ..Default::default()
        });
        let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
        let hook = Mutex::new(Some(hook));
        let last_sync = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 4,
                server_version: 4,
                chunks: 1,
                bytes: 1024,
                last_failure: None,
            }),
            receive: None,
        };
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.last_sync = Some(last_sync.clone());
            runtime.last_sync_at_ms = Some(42);
            runtime.last_error = Some("previous error".to_string());
        }

        let result = sync_cloudsync_connection(
            db.pool(),
            &db.cloudsync_connection,
            &db.cloudsync_interrupt,
            &db.cloudsync_sync_operation,
            &db.cloudsync_runtime,
            &hook,
        )
        .await;

        assert!(matches!(result, Ok(CloudsyncStepOutcome::Deferred)));
        assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 0);
        assert!(recording_hook.after_result.lock().unwrap().is_none());
        let pending_after = {
            let mut connection = db.pool().acquire().await.unwrap();
            crate::cloudsync::ops::ensure_pending_payload_fits(
                &mut connection,
                &db.cloudsync_interrupt,
            )
            .await
            .unwrap()
        };
        assert_eq!(pending_after, pending_before);
        let runtime = db.cloudsync_runtime.lock().unwrap();
        assert_eq!(runtime.last_sync.as_ref(), Some(&last_sync));
        assert_eq!(runtime.last_sync_at_ms, Some(42));
        assert_eq!(runtime.last_error.as_deref(), Some("previous error"));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn activity_pause_during_pending_preflight_defers_and_drains_before_local_write() {
        let db = Arc::new(Db::connect_memory().await.unwrap());
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("items", None, None).await.unwrap();
        sqlx::query("SELECT cloudsync_set('payload_max_chunk_size', '33554432')")
            .fetch_optional(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "WITH RECURSIVE rows(id) AS (
                SELECT 1
                UNION ALL
                SELECT id + 1 FROM rows WHERE id < 100000
            )
            INSERT INTO items (id, value)
            SELECT printf('item-%d', id), printf('value-%d', id) FROM rows",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let recording_hook = Arc::new(RecordingSyncHook::default());
        db.set_cloudsync_sync_hook(recording_hook.clone());
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
        }

        let request_db = Arc::clone(&db);
        let request = tokio::spawn(async move { request_db.cloudsync_trigger_sync().await });
        let started_at = std::time::Instant::now();
        while !db.cloudsync_interrupt_registered() {
            assert!(
                !request.is_finished(),
                "pending payload generation completed before interrupt registration was observable"
            );
            assert!(
                started_at.elapsed() < Duration::from_secs(2),
                "pending payload generation never registered for interruption"
            );
            tokio::task::yield_now().await;
        }

        recording_hook.activity_paused.store(true, Ordering::SeqCst);
        while !request.is_finished() {
            db.cloudsync_interrupt_sync();
            assert!(
                started_at.elapsed() < Duration::from_secs(2),
                "pending payload generation did not honor sqlite3_interrupt"
            );
            tokio::task::yield_now().await;
        }

        assert_eq!(
            request.await.unwrap().unwrap(),
            CloudsyncNetworkResult::default()
        );
        assert!(db.cloudsync_runtime.lock().unwrap().last_error.is_none());
        assert_eq!(recording_hook.before_calls.load(Ordering::SeqCst), 0);
        assert!(recording_hook.after_result.lock().unwrap().is_none());
        assert!(!db.cloudsync_interrupt_registered());
        tokio::time::timeout(
            Duration::from_millis(250),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect("pending payload worker did not become idle after activity pause");
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
                .execute(db.pool()),
        )
        .await
        .expect("immediate local write remained blocked after activity pause")
        .unwrap();
    }

    #[tokio::test]
    async fn paused_manual_sync_preserves_the_last_result_and_error() {
        let db = Db::connect_memory().await.unwrap();
        let hook = Arc::new(RecordingSyncHook {
            activity_paused: AtomicBool::new(true),
            ..Default::default()
        });
        db.set_cloudsync_sync_hook(hook);
        db.cloudsync_configure(test_cloudsync_config())
            .await
            .unwrap();
        let last_sync = CloudsyncNetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 3,
                tables: vec!["sessions".to_string()],
                chunks: 1,
                bytes: 2048,
                complete: true,
                error: None,
                last_failure: None,
            }),
        };
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.network_initialized = true;
            runtime.last_sync = Some(last_sync.clone());
            runtime.last_sync_at_ms = Some(42);
            runtime.last_error = Some("previous error".to_string());
            runtime.consecutive_failures = 2;
        }

        assert_eq!(
            db.cloudsync_trigger_sync().await.unwrap(),
            CloudsyncNetworkResult::default()
        );
        let status = db.cloudsync_status().await.unwrap();
        assert!(status.activity_paused);
        assert_eq!(status.has_unsent_changes, None);
        assert_eq!(status.last_sync.as_ref(), Some(&last_sync));
        assert_eq!(status.last_sync_at_ms, Some(42));
        assert_eq!(status.last_error.as_deref(), Some("previous error"));
        assert_eq!(status.consecutive_failures, 2);
    }

    #[tokio::test]
    async fn sync_idle_barrier_waits_for_the_active_operation() {
        let db = Db::connect_memory_plain().await.unwrap();
        let operation = db.cloudsync_sync_operation.lock().await;
        let mut barrier = Box::pin(db.cloudsync_wait_for_sync_idle());

        tokio::select! {
            biased;
            () = &mut barrier => panic!("sync idle barrier bypassed the active operation"),
            _ = tokio::task::yield_now() => {}
        }

        drop(operation);
        tokio::time::timeout(Duration::from_millis(100), barrier)
            .await
            .expect("sync idle barrier did not finish");
    }

    #[tokio::test]
    async fn requested_sync_wakes_are_coalesced() {
        let db = Db::connect_memory_plain().await.unwrap();

        db.cloudsync_request_sync();
        assert!(
            tokio::time::timeout(
                Duration::from_millis(10),
                db.cloudsync_sync_requested.notified(),
            )
            .await
            .is_err(),
            "a stopped runtime retained a requested sync wake"
        );
        db.cloudsync_runtime.lock().unwrap().running = true;
        db.cloudsync_request_sync();
        db.cloudsync_request_sync();
        tokio::time::timeout(
            Duration::from_millis(100),
            db.cloudsync_sync_requested.notified(),
        )
        .await
        .expect("queued sync wake was lost");
        assert!(
            tokio::time::timeout(
                Duration::from_millis(10),
                db.cloudsync_sync_requested.notified(),
            )
            .await
            .is_err(),
            "duplicate sync wakes were not coalesced"
        );
    }

    #[tokio::test]
    async fn requested_sync_interrupts_and_consumes_the_retry_backoff() {
        let sync_requested = tokio::sync::Notify::new();
        let (_shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        sync_requested.notify_one();
        sync_requested.notify_one();
        tokio::time::timeout(
            Duration::from_millis(100),
            wait_for_retry_request_or_shutdown(
                Duration::from_secs(60),
                &sync_requested,
                &mut shutdown_rx,
            ),
        )
        .await
        .expect("requested sync did not interrupt retry backoff");
        assert!(
            tokio::time::timeout(
                Duration::from_millis(10),
                wait_for_retry_request_or_shutdown(
                    Duration::from_secs(60),
                    &sync_requested,
                    &mut shutdown_rx,
                ),
            )
            .await
            .is_err(),
            "the retry wake was left queued for a duplicate sync"
        );
    }

    #[test]
    fn requested_sync_retries_promptly_when_another_sync_is_busy() {
        let interval = Duration::from_secs(30);
        let request_pending = cloudsync_request_pending(false, CloudsyncWake::Requested);
        let request_pending = cloudsync_request_pending(request_pending, CloudsyncWake::Interval);

        assert_eq!(
            cloudsync_busy_delay(request_pending, interval),
            CLOUDSYNC_PROGRESS_INTERVAL,
            "a timer collision must preserve the pending requested sync"
        );
        assert_eq!(cloudsync_busy_delay(false, interval), interval);
    }

    #[tokio::test]
    async fn after_sync_hook_receives_the_bounded_network_result() {
        let db = Db::connect_memory_plain().await.unwrap();
        let expected = CloudsyncNetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 4,
                server_version: 4,
                chunks: 1,
                bytes: 1024,
                last_failure: None,
            }),
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 3,
                tables: vec!["sessions".to_string()],
                chunks: 1,
                bytes: 2048,
                complete: false,
                error: None,
                last_failure: None,
            }),
        };
        let recording_hook = Arc::new(RecordingSyncHook::default());
        let hook: Arc<dyn crate::CloudsyncSyncHook> = recording_hook.clone();
        let hook = Mutex::new(Some(hook));

        run_after_sync_hook(&hook, db.pool(), &expected)
            .await
            .unwrap();

        assert_eq!(
            recording_hook.after_result.lock().unwrap().as_ref(),
            Some(&expected)
        );
    }

    #[tokio::test]
    async fn logout_releases_connection_after_partial_startup() {
        let mut db = Db::connect_memory_plain().await.unwrap();
        db.cloudsync_enabled = true;
        db.cloudsync_configure(test_cloudsync_config())
            .await
            .unwrap();
        *db.cloudsync_connection.lock().await = Some(db.pool.acquire().await.unwrap());
        db.cloudsync_runtime.lock().unwrap().outbound_work_state = Some(false);

        db.cloudsync_logout(false).await.unwrap();

        assert!(db.cloudsync_connection.lock().await.is_none());
        let runtime = db.cloudsync_runtime.lock().unwrap();
        assert!(runtime.config.is_none());
        assert!(runtime.outbound_work_state.is_none());
    }

    #[tokio::test]
    async fn logout_signals_shutdown_before_waiting_for_an_active_sync_operation() {
        let mut db = Db::connect_memory_plain().await.unwrap();
        db.cloudsync_enabled = true;
        db.cloudsync_configure(test_cloudsync_config())
            .await
            .unwrap();
        let db = Arc::new(db);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (shutdown_observed_tx, shutdown_observed_rx) = oneshot::channel();
        let join_handle = tokio::spawn(async move {
            shutdown_rx.await.unwrap();
            let _ = shutdown_observed_tx.send(());
        });
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = true;
            runtime.task = Some(CloudsyncBackgroundTask {
                shutdown_tx: Some(shutdown_tx),
                join_handle,
            });
        }
        let sync_operation = db.cloudsync_sync_operation.lock().await;
        let logout_db = Arc::clone(&db);
        let logout = tokio::spawn(async move { logout_db.cloudsync_logout(false).await });

        tokio::time::timeout(Duration::from_secs(1), shutdown_observed_rx)
            .await
            .expect("logout waited for the sync operation before signaling shutdown")
            .unwrap();
        assert!(!logout.is_finished());

        drop(sync_operation);
        logout.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn task_only_resume_reuses_initialized_transport_and_preserves_error_state() {
        let mut db = Db::connect_memory_plain().await.unwrap();
        db.cloudsync_enabled = true;
        let config = test_cloudsync_config();
        db.cloudsync_configure(config.clone()).await.unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.network_initialized = true;
            runtime.last_error = Some("existing sync failure".to_string());
            runtime.consecutive_failures = 2;
        }
        let sync_operation = db.cloudsync_sync_operation.lock().await;

        db.start_cloudsync_background_task(&config);

        {
            let runtime = db.cloudsync_runtime.lock().unwrap();
            assert!(runtime.running);
            assert!(runtime.network_initialized);
            assert!(runtime.task.is_some());
            assert_eq!(runtime.last_error.as_deref(), Some("existing sync failure"));
            assert_eq!(runtime.consecutive_failures, 2);
        }
        db.stop_cloudsync_task().await;
        drop(sync_operation);
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
    async fn manual_transport_initializes_without_starting_background_sync() {
        let db = Db::connect_memory().await.unwrap();
        sqlx::query("CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();
        let mut config = test_cloudsync_config();
        config.tables = vec![CloudsyncTableSpec {
            table_name: "items".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }];

        db.cloudsync_prepare_manual_transport(config).await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(status.configured);
        assert!(!status.running);
        assert!(status.network_initialized);
        assert!(db.cloudsync_runtime.lock().unwrap().task.is_none());
        let pending = db.cloudsync_manual_pending_payload_batch().await.unwrap();
        assert_eq!(pending.chunks, 0);
        assert!(pending.complete);
        assert!(pending.fits);
        db.cloudsync_suspend().await.unwrap();
    }

    #[tokio::test]
    async fn manual_transport_refuses_to_take_over_a_running_runtime() {
        let db = Db::connect_memory().await.unwrap();
        db.cloudsync_runtime.lock().unwrap().running = true;

        let error = db
            .cloudsync_prepare_manual_transport(test_cloudsync_config())
            .await
            .unwrap_err();

        assert!(matches!(error, CloudsyncRuntimeError::RestartRequired));
        db.cloudsync_runtime.lock().unwrap().running = false;
    }

    #[tokio::test]
    async fn prepared_transport_resumes_without_reinitializing_the_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&dir.path().join("app.db")),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        let config = test_cloudsync_config();
        db.cloudsync_prepare_manual_transport(config).await.unwrap();
        {
            let mut connection = db.cloudsync_connection.lock().await;
            sqlx::query("CREATE TEMP TABLE transport_marker (value INTEGER)")
                .execute(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }
        let sync_operation = db.cloudsync_sync_operation.lock().await;
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.last_error = Some("stale recovery error".to_string());
            runtime.last_error_kind = Some(hypr_cloudsync::ErrorKind::Transient);
            runtime.consecutive_failures = 3;
            runtime.outbound_work_state = Some(true);
        }

        db.cloudsync_resume_prepared_transport().await.unwrap();

        let marker_exists: i64 = {
            let mut connection = db.cloudsync_connection.lock().await;
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'transport_marker'",
            )
            .fetch_one(&mut **connection.as_mut().unwrap())
            .await
            .unwrap()
        };
        assert_eq!(marker_exists, 1);
        let status = db.cloudsync_status().await.unwrap();
        assert!(status.running);
        assert!(status.network_initialized);
        assert!(status.last_error.is_none());
        assert!(status.last_error_kind.is_none());
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.has_unsent_changes, None);

        db.stop_cloudsync_task().await;
        drop(sync_operation);
        db.cloudsync_suspend().await.unwrap();
    }

    #[tokio::test]
    async fn prepared_transport_reaps_a_finished_background_task_before_resuming() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&dir.path().join("app.db")),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        db.cloudsync_prepare_manual_transport(test_cloudsync_config())
            .await
            .unwrap();
        let sync_operation = db.cloudsync_sync_operation.lock().await;
        let join_handle = tokio::spawn(async {});
        while !join_handle.is_finished() {
            tokio::task::yield_now().await;
        }
        let (shutdown_tx, _shutdown_rx) = oneshot::channel();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.running = false;
            runtime.task = Some(CloudsyncBackgroundTask {
                shutdown_tx: Some(shutdown_tx),
                join_handle,
            });
        }

        db.cloudsync_resume_prepared_transport().await.unwrap();

        {
            let runtime = db.cloudsync_runtime.lock().unwrap();
            assert!(runtime.running);
            assert!(runtime.network_initialized);
            assert!(runtime.task.is_some());
        }

        db.stop_cloudsync_task().await;
        drop(sync_operation);
        db.cloudsync_suspend().await.unwrap();
    }

    #[tokio::test]
    async fn configure_rejects_mutation_while_manual_transport_is_prepared() {
        let db = Db::connect_memory().await.unwrap();
        let original = test_cloudsync_config();
        db.cloudsync_prepare_manual_transport(original.clone())
            .await
            .unwrap();
        let mut replacement = original.clone();
        replacement.connection_string = "replacement-managed-database-id".to_string();

        let error = db.cloudsync_configure(replacement).await.unwrap_err();

        assert!(matches!(error, CloudsyncRuntimeError::RestartRequired));
        assert_eq!(
            db.cloudsync_runtime.lock().unwrap().config.as_ref(),
            Some(&original)
        );
        db.cloudsync_suspend().await.unwrap();
    }

    #[tokio::test]
    async fn reconfigure_cleans_up_a_prepared_manual_transport() {
        let db = Db::connect_memory().await.unwrap();
        db.cloudsync_prepare_manual_transport(test_cloudsync_config())
            .await
            .unwrap();
        let mut replacement = test_cloudsync_config();
        replacement.connection_string = "replacement-managed-database-id".to_string();

        db.cloudsync_reconfigure(replacement.clone()).await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(status.configured);
        assert!(!status.running);
        assert!(!status.network_initialized);
        assert_eq!(
            db.cloudsync_runtime.lock().unwrap().config.as_ref(),
            Some(&replacement)
        );
        db.cloudsync_suspend().await.unwrap();
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
    async fn status_stays_observable_while_mutations_wait_for_lifecycle() {
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

        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("status blocked on the lifecycle lock")
            .unwrap();
        assert!(status.configured);
        assert!(!status.running);
        assert_eq!(status.has_unsent_changes, None);

        let mut trigger = Box::pin(db.cloudsync_trigger_sync());
        tokio::select! {
            biased;
            result = &mut trigger => panic!("manual sync bypassed lifecycle lock: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }

        drop(lifecycle);
        suspend.await.unwrap();
        assert_eq!(trigger.await.unwrap(), CloudsyncNetworkResult::default());
        assert!(!db.cloudsync_status().await.unwrap().configured);
    }

    #[tokio::test]
    async fn status_preserves_unknown_outbound_state_during_sync_preflight() {
        let db = Db::connect_memory().await.unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
        }
        let _sync_operation = db.cloudsync_sync_operation.lock().await;

        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("status blocked on the active sync operation")
            .unwrap();

        assert!(status.configured);
        assert!(status.running);
        assert!(status.network_initialized);
        assert_eq!(status.has_unsent_changes, None);
    }

    #[tokio::test]
    async fn status_reports_receive_only_work_while_a_sync_operation_is_running() {
        let db = Db::connect_memory().await.unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
            runtime.last_sync_at_ms = Some(42);
            runtime.outbound_work_state = Some(false);
        }
        let _sync_operation = db.cloudsync_sync_operation.lock().await;

        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("status blocked on the active receive")
            .unwrap();

        assert_eq!(status.has_unsent_changes, Some(false));
        assert_eq!(status.last_sync_at_ms, Some(42));
    }

    #[tokio::test]
    async fn status_reports_outbound_work_while_a_sync_operation_is_running() {
        let db = Db::connect_memory().await.unwrap();
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
            runtime.last_sync_at_ms = Some(42);
            runtime.outbound_work_state = Some(true);
        }
        let _sync_operation = db.cloudsync_sync_operation.lock().await;

        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("status blocked on the active send")
            .unwrap();

        assert_eq!(status.has_unsent_changes, Some(true));
    }

    #[tokio::test]
    async fn status_reads_unsent_changes_without_network_io() {
        let (_dir, db) = db_with_local_unsent_changes().await;
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
        }

        let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
            .await
            .expect("local cloudsync status blocked")
            .unwrap();

        assert_eq!(status.has_unsent_changes, Some(true));
    }

    #[tokio::test]
    async fn repeated_status_polling_does_not_queue_work_on_a_busy_pool() {
        let (_dir, db) = db_with_local_unsent_changes().await;
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.running = true;
            runtime.network_initialized = true;
        }
        let mut first_connection = db.pool().acquire().await.unwrap();

        for _ in 0..32 {
            let status = tokio::time::timeout(Duration::from_millis(50), db.cloudsync_status())
                .await
                .expect("CloudSync status waited for a busy pool")
                .unwrap();
            assert_eq!(status.has_unsent_changes, None);
        }

        first_connection.return_to_pool().await;
        for _ in 0..32 {
            let status = tokio::time::timeout(Duration::from_millis(100), db.cloudsync_status())
                .await
                .expect("CloudSync status left SQLite work in flight")
                .unwrap();
            assert_eq!(status.has_unsent_changes, Some(true));
        }
        assert!(db.pool().num_idle() > 0);
        tokio::time::timeout(Duration::from_millis(100), db.pool().acquire())
            .await
            .expect("repeated CloudSync status polling exhausted the pool")
            .unwrap();
    }

    #[tokio::test]
    async fn logout_checks_unsent_changes_without_network_io() {
        let (_dir, db) = db_with_local_unsent_changes().await;
        {
            let mut runtime = db.cloudsync_runtime.lock().unwrap();
            runtime.config = Some(test_cloudsync_config());
            runtime.network_initialized = true;
        }

        let error = db.cloudsync_logout(false).await.unwrap_err();

        assert!(
            matches!(error, CloudsyncRuntimeError::UnsentChanges),
            "{error:?}"
        );
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
            let sync_requested = tokio::sync::Notify::new();
            let _ = retry_started_tx.send(());
            assert!(
                !wait_for_retry_request_or_shutdown(
                    Duration::from_secs(60),
                    &sync_requested,
                    &mut shutdown_rx,
                )
                .await
            );
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
    async fn shutdown_interrupts_an_active_sync_future() {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        shutdown_tx.send(()).unwrap();

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_or_shutdown(pending::<()>(), &mut shutdown_rx),
        )
        .await
        .expect("active sync ignored shutdown");

        assert!(result.is_none());
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
        db.cloudsync_runtime.lock().unwrap().outbound_work_state = Some(false);
        db.cloudsync_suspend().await.unwrap();

        let status = db.cloudsync_status().await.unwrap();
        assert!(!status.configured);
        assert!(!status.running);
        assert!(!status.network_initialized);
        assert!(
            db.cloudsync_runtime
                .lock()
                .unwrap()
                .outbound_work_state
                .is_none()
        );
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
const CLOUDSYNC_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

struct CloudsyncStepResult {
    network: CloudsyncNetworkResult,
    local_work_remaining: bool,
}

enum CloudsyncStepOutcome {
    Completed(Box<CloudsyncStepResult>),
    Deferred,
}

#[derive(Clone, Copy)]
struct CloudsyncLoopConfig {
    interval: Duration,
}

struct CloudsyncLoopContext {
    pool: SqlitePool,
    connection: Arc<tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>>,
    interrupt: Arc<super::CloudsyncInterruptHandle>,
    sync_operation: Arc<tokio::sync::Mutex<()>>,
    sync_requested: Arc<tokio::sync::Notify>,
    runtime_state: Arc<Mutex<CloudsyncRuntimeState>>,
    sync_hook: Arc<Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>>,
    config: CloudsyncLoopConfig,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CloudsyncWake {
    Interval,
    Requested,
}

fn cloudsync_request_pending(request_pending: bool, wake: CloudsyncWake) -> bool {
    request_pending || wake == CloudsyncWake::Requested
}

fn cloudsync_busy_delay(request_pending: bool, interval: Duration) -> Duration {
    if request_pending {
        CLOUDSYNC_PROGRESS_INTERVAL
    } else {
        interval
    }
}

async fn cloudsync_background_loop(
    context: CloudsyncLoopContext,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut next_sync_delay = cloudsync_next_delay(None, false, context.config.interval);
    let mut request_pending = false;
    loop {
        let wake = tokio::select! {
            biased;
            _ = &mut shutdown_rx => break,
            _ = context.sync_requested.notified() => CloudsyncWake::Requested,
            _ = tokio::time::sleep(next_sync_delay) => CloudsyncWake::Interval,
        };
        request_pending = cloudsync_request_pending(request_pending, wake);
        let Ok(sync_available) = context.sync_operation.try_lock() else {
            tracing::debug!("cloudsync interval skipped because another sync is active");
            next_sync_delay = cloudsync_busy_delay(request_pending, context.config.interval);
            continue;
        };
        drop(sync_available);
        request_pending = false;
        let Some(result) = sync_cloudsync_with_retry(&context, &mut shutdown_rx).await else {
            break;
        };

        match result {
            Ok(CloudsyncStepOutcome::Completed(step)) => {
                next_sync_delay = cloudsync_next_delay(
                    Some(&step.network),
                    step.local_work_remaining,
                    context.config.interval,
                );
                record_sync_result(
                    &context.runtime_state,
                    step.network,
                    step.local_work_remaining,
                );
            }
            Ok(CloudsyncStepOutcome::Deferred) => {
                next_sync_delay = context.config.interval;
            }
            Err(error) => {
                let kind = error.kind();
                let mut runtime = context.runtime_state.lock().unwrap();
                runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                runtime.last_error = Some(error.to_string());
                runtime.last_error_kind = Some(kind);
                runtime.running = false;
                break;
            }
        }
    }
}

async fn sync_cloudsync_with_retry(
    context: &CloudsyncLoopContext,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<Result<CloudsyncStepOutcome, hypr_cloudsync::Error>> {
    let mut backoff = ExponentialBuilder::default()
        .with_min_delay(context.config.interval)
        .with_max_delay(Duration::from_secs(MAX_BACKOFF_SECS))
        .with_jitter()
        .build();

    loop {
        let result = run_sync_or_shutdown(context, shutdown_rx).await?;

        match result {
            Err(error) if error.kind() == hypr_cloudsync::ErrorKind::Transient => {
                let Some(retry_after) = backoff.next() else {
                    return Some(Err(error));
                };

                let failures = {
                    let mut runtime = context.runtime_state.lock().unwrap();
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

                if !wait_for_retry_request_or_shutdown(
                    retry_after,
                    &context.sync_requested,
                    shutdown_rx,
                )
                .await
                {
                    return None;
                }
            }
            result => return Some(result),
        }
    }
}

async fn run_sync_or_shutdown(
    context: &CloudsyncLoopContext,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<Result<CloudsyncStepOutcome, hypr_cloudsync::Error>> {
    let sync = sync_cloudsync_connection(
        &context.pool,
        &context.connection,
        &context.interrupt,
        &context.sync_operation,
        &context.runtime_state,
        &context.sync_hook,
    );
    tokio::pin!(sync);

    tokio::select! {
        biased;
        _ = &mut *shutdown_rx => {
            cancel_active_sync_hook(&context.sync_hook);
            let mut interrupt_interval = tokio::time::interval(Duration::from_millis(25));
            loop {
                tokio::select! {
                    biased;
                    _ = &mut sync => return None,
                    _ = interrupt_interval.tick() => {
                        cancel_active_sync_hook(&context.sync_hook);
                        context.interrupt.interrupt();
                    }
                }
            }
        }
        result = &mut sync => Some(result),
    }
}

#[cfg(test)]
async fn run_or_shutdown<T>(
    future: impl Future<Output = T>,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<T> {
    tokio::select! {
        biased;
        _ = &mut *shutdown_rx => None,
        result = future => Some(result),
    }
}

async fn wait_for_retry_request_or_shutdown(
    retry_after: Duration,
    sync_requested: &tokio::sync::Notify,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> bool {
    tokio::select! {
        biased;
        _ = &mut *shutdown_rx => false,
        _ = sync_requested.notified() => true,
        _ = tokio::time::sleep(retry_after) => true,
    }
}

async fn sync_cloudsync_connection(
    pool: &SqlitePool,
    connection: &tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>,
    interrupt: &super::CloudsyncInterruptHandle,
    sync_operation: &tokio::sync::Mutex<()>,
    runtime_state: &Mutex<CloudsyncRuntimeState>,
    sync_hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
) -> Result<CloudsyncStepOutcome, hypr_cloudsync::Error> {
    let _sync_operation = sync_operation.lock().await;
    if cloudsync_activity_paused(sync_hook) {
        tracing::debug!("cloudsync deferred while local activity is active");
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let pending_batch = {
        let mut connection = connection.lock().await;
        if connection.is_none() {
            *connection = Some(pool.acquire().await?);
        }
        let result =
            super::ops::ensure_pending_payload_fits(connection.as_mut().unwrap(), interrupt).await;
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        result
    };
    let pending_batch = match pending_batch {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    let directive = if pending_batch.chunks > 0 {
        super::CloudsyncSyncDirective::SendAndReceive
    } else {
        run_before_sync_hook(sync_hook, pool).await?
    };
    if directive == super::CloudsyncSyncDirective::Deferred {
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    if cloudsync_activity_paused(sync_hook) {
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let mut connection = connection.lock().await;
    if connection.is_none() {
        *connection = Some(pool.acquire().await?);
    }
    let has_outbound_work = match directive {
        super::CloudsyncSyncDirective::SendAndReceive if pending_batch.chunks == 0 => {
            let result =
                super::ops::ensure_pending_payload_fits(connection.as_mut().unwrap(), interrupt)
                    .await;
            match result {
                Err(_) if cloudsync_activity_paused(sync_hook) => {
                    if pool.options().get_max_connections() == 1 {
                        connection.take();
                    }
                    return Ok(CloudsyncStepOutcome::Deferred);
                }
                result => result?.chunks > 0,
            }
        }
        super::CloudsyncSyncDirective::SendAndReceive => true,
        super::CloudsyncSyncDirective::ReceiveOnly => false,
        super::CloudsyncSyncDirective::Deferred => unreachable!("deferred before native sync"),
    };
    runtime_state.lock().unwrap().outbound_work_state = Some(has_outbound_work);
    if cloudsync_activity_paused(sync_hook) {
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let send = match directive {
        super::CloudsyncSyncDirective::SendAndReceive => {
            super::ops::guarded_interruptible_network_send_changes(
                connection.as_mut().unwrap(),
                interrupt,
                || cloudsync_activity_paused(sync_hook),
            )
            .await
        }
        super::CloudsyncSyncDirective::ReceiveOnly => Ok(CloudsyncNetworkResult::default()),
        super::CloudsyncSyncDirective::Deferred => unreachable!("deferred before native sync"),
    };
    let send = match send {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            if pool.options().get_max_connections() == 1 {
                connection.take();
            }
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    if sync_send_settled(&send) {
        runtime_state.lock().unwrap().outbound_work_state = Some(false);
    }
    if cloudsync_activity_paused(sync_hook) {
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let receive =
        super::ops::interruptible_network_receive_changes(connection.as_mut().unwrap(), interrupt)
            .await;
    let receive = match receive {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            if pool.options().get_max_connections() == 1 {
                connection.take();
            }
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    let result = merge_bounded_sync_results(send, receive);
    if pool.options().get_max_connections() == 1 {
        connection.take();
    }
    drop(connection);
    let outcome = run_after_sync_hook(sync_hook, pool, &result).await?;
    if outcome.deferred || cloudsync_activity_paused(sync_hook) {
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    runtime_state.lock().unwrap().outbound_work_state = Some(outcome.local_work_remaining);
    Ok(CloudsyncStepOutcome::Completed(Box::new(
        CloudsyncStepResult {
            network: result,
            local_work_remaining: outcome.local_work_remaining,
        },
    )))
}

pub(super) fn cloudsync_activity_paused(
    hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
) -> bool {
    hook.lock()
        .unwrap()
        .as_ref()
        .is_some_and(|hook| hook.activity_paused())
}

fn cancel_active_sync_hook(hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>) {
    if let Some(hook) = hook.lock().unwrap().clone() {
        hook.cancel_active_sync();
    }
}

fn merge_bounded_sync_results(
    send: CloudsyncNetworkResult,
    receive: CloudsyncNetworkResult,
) -> CloudsyncNetworkResult {
    CloudsyncNetworkResult {
        send: send.send.or(receive.send),
        receive: receive.receive.or(send.receive),
    }
}

async fn run_before_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
    pool: &SqlitePool,
) -> Result<super::CloudsyncSyncDirective, hypr_cloudsync::Error> {
    let hook = hook.lock().unwrap().clone();
    match hook {
        Some(hook) => hook.before_sync(pool).await,
        None => Ok(super::CloudsyncSyncDirective::default()),
    }
}

async fn run_after_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::CloudsyncSyncHook>>>,
    pool: &SqlitePool,
    result: &CloudsyncNetworkResult,
) -> Result<super::CloudsyncHookOutcome, hypr_cloudsync::Error> {
    let hook = hook.lock().unwrap().clone();
    match hook {
        Some(hook) => hook.after_sync(pool, result).await,
        None => Ok(super::CloudsyncHookOutcome::default()),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
