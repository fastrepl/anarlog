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
    ) -> Result<Option<bool>, anlg_cloudsync::Error> {
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
) -> Result<(), anlg_cloudsync::Error>
where
    A: FnOnce() -> AF,
    AF: Future<Output = Result<(), anlg_cloudsync::Error>>,
    C: FnOnce() -> CF,
    CF: Future<Output = Result<(), anlg_cloudsync::Error>>,
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
            .map(|error| anlg_cloudsync::Error::Io(std::io::Error::other(error)).kind());
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
mod tests;

fn record_sync_error(runtime: &Mutex<CloudsyncRuntimeState>, error: &anlg_cloudsync::Error) {
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
) -> Option<Result<CloudsyncStepOutcome, anlg_cloudsync::Error>> {
    let mut backoff = ExponentialBuilder::default()
        .with_min_delay(context.config.interval)
        .with_max_delay(Duration::from_secs(MAX_BACKOFF_SECS))
        .with_jitter()
        .build();

    loop {
        let result = run_sync_or_shutdown(context, shutdown_rx).await?;

        match result {
            Err(error) if error.kind() == anlg_cloudsync::ErrorKind::Transient => {
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
) -> Option<Result<CloudsyncStepOutcome, anlg_cloudsync::Error>> {
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
) -> Result<CloudsyncStepOutcome, anlg_cloudsync::Error> {
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
) -> Result<super::CloudsyncSyncDirective, anlg_cloudsync::Error> {
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
) -> Result<super::CloudsyncHookOutcome, anlg_cloudsync::Error> {
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
