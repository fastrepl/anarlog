use std::future::Future;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::time::Duration;

#[cfg(test)]
use sqlx::SqlitePool;
use tokio::sync::oneshot;

mod background;

pub(super) use background::cloudsync_activity_paused;
#[cfg(test)]
use background::{
    CLOUDSYNC_PROGRESS_INTERVAL, CloudsyncWake, MAX_ACTIVITY_LOG_ENTRIES, cloudsync_busy_delay,
    cloudsync_next_delay, cloudsync_request_pending, cloudsync_wake_deadline,
    drain_pending_changes, merge_bounded_sync_results, next_synced_change, run_after_sync_hook,
    run_before_sync_hook, run_or_shutdown, sync_result_needs_receive_progress,
    wait_for_retry_request_or_shutdown,
};
use background::{
    CloudsyncLoopConfig, CloudsyncLoopContext, CloudsyncStepOutcome, cancel_active_sync_hook,
    cloudsync_background_loop, record_sync_error, record_sync_result, sync_cloudsync_connection,
};

use super::state::CloudsyncBackgroundTask;
#[cfg(test)]
use super::state::CloudsyncRuntimeState;
use super::types::{
    CloudsyncActivityTrigger, CloudsyncErrorKind, CloudsyncNetworkResult, CloudsyncRuntimeConfig,
    CloudsyncRuntimeError, CloudsyncStatus,
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
            change_rx: self.change_notifier().subscribe(),
            // Every registry table, enabled or not: in E2EE mode the app tables
            // are disabled (only e2ee_records syncs directly) yet their writes
            // must still wake the loop, since the reconcile hook picks them up
            // at sync time.
            synced_tables: config
                .tables
                .iter()
                .map(|table| table.table_name.to_ascii_lowercase())
                .collect(),
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
            runtime.activity_log.clear();
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
            runtime.activity_log.clear();
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
            activity_log,
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
                runtime.activity_log.iter().rev().cloned().collect(),
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
            activity_log,
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
                    CloudsyncActivityTrigger::Manual,
                );
                Ok(step.network)
            }
            Ok(CloudsyncStepOutcome::Deferred) => Ok(CloudsyncNetworkResult::default()),
            Err(error) => {
                record_sync_error(
                    &self.cloudsync_runtime,
                    &error,
                    CloudsyncActivityTrigger::Manual,
                );
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

#[cfg(test)]
mod tests;
