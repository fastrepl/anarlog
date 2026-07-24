use sqlx::pool::PoolConnection;
use sqlx::{Executor, Sqlite, SqliteConnection};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::MutexGuard;

use super::{CloudsyncAuth, CloudsyncRuntimeError, CloudsyncTableSpec};
use crate::Db;

pub const CLOUDSYNC_MAX_OUTBOUND_CHUNKS: u32 = 8;
pub const CLOUDSYNC_MAX_OUTBOUND_ROWS: u64 = 4_096;
pub const CLOUDSYNC_MAX_OUTBOUND_BYTES: u64 = 32 * 1024 * 1024;
const CLOUDSYNC_POOL_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

impl Db {
    async fn lock_cloudsync_connection(
        &self,
    ) -> Result<MutexGuard<'_, Option<PoolConnection<Sqlite>>>, hypr_cloudsync::Error> {
        let mut connection = self.cloudsync_connection.lock().await;
        if connection.is_none() {
            *connection = Some(self.pool.acquire().await?);
        }
        Ok(connection)
    }

    fn release_single_pool_connection(
        &self,
        connection: &mut MutexGuard<'_, Option<PoolConnection<Sqlite>>>,
    ) {
        if self.pool.options().get_max_connections() == 1 {
            connection.take();
        }
    }

    pub fn cloudsync_enabled(&self) -> bool {
        self.cloudsync_enabled
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&std::path::Path> {
        self.cloudsync_path.as_deref()
    }

    pub async fn cloudsync_version(&self) -> Result<String, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        init_flags: Option<i64>,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_init(
            connection.as_mut().unwrap(),
            table_name,
            crdt_algo,
            init_flags,
            &self.cloudsync_interrupt,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_set_filter(
        &self,
        table_name: &str,
        filter_expression: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::set_filter(
            &mut **connection.as_mut().unwrap(),
            table_name,
            filter_expression,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub(crate) async fn cloudsync_init_enabled_tables(
        &self,
        tables: &[CloudsyncTableSpec],
    ) -> Result<(), CloudsyncRuntimeError> {
        if !tables.iter().any(|table| table.enabled) {
            return Ok(());
        }

        let acquisition_deadline = tokio::time::Instant::now() + CLOUDSYNC_POOL_DRAIN_TIMEOUT;
        let mut pinned =
            tokio::time::timeout_at(acquisition_deadline, self.cloudsync_connection.lock())
                .await
                .map_err(|_| CloudsyncRuntimeError::LocalStatusBusy)?;
        if pinned.is_none() {
            *pinned = Some(
                tokio::time::timeout_at(acquisition_deadline, self.pool.acquire())
                    .await
                    .map_err(|_| CloudsyncRuntimeError::LocalStatusBusy)?
                    .map_err(hypr_cloudsync::Error::from)?,
            );
        }

        let mut connections = Vec::new();
        for _ in 1..self.pool.options().get_max_connections() {
            match tokio::time::timeout_at(acquisition_deadline, self.pool.acquire()).await {
                Ok(Ok(connection)) => connections.push(connection),
                Ok(Err(error)) => {
                    return_pool_connections(connections).await;
                    self.release_single_pool_connection(&mut pinned);
                    return Err(hypr_cloudsync::Error::from(error).into());
                }
                Err(_) => {
                    return_pool_connections(connections).await;
                    self.release_single_pool_connection(&mut pinned);
                    return Err(CloudsyncRuntimeError::LocalStatusBusy);
                }
            }
        }

        let result: Result<(), hypr_cloudsync::Error> = async {
            init_enabled_tables(pinned.as_mut().unwrap(), tables, &self.cloudsync_interrupt)
                .await?;
            for connection in &mut connections {
                init_enabled_tables(connection, tables, &self.cloudsync_interrupt).await?;
            }
            Ok(())
        }
        .await;

        if result.is_ok() {
            self.cloudsync_initializer.replace_tables(
                tables
                    .iter()
                    .filter(|table| table.enabled)
                    .cloned()
                    .collect(),
            );
        }

        self.release_single_pool_connection(&mut pinned);
        match result {
            Ok(()) => {
                return_pool_connections(connections).await;
                Ok(())
            }
            Err(error) => {
                for connection in connections {
                    let _ = connection.close().await;
                }
                Err(error.into())
            }
        }
    }

    pub async fn cloudsync_network_init(
        &self,
        connection_string: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_init(&mut **connection.as_mut().unwrap(), connection_string)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_apikey(
        &self,
        api_key: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_set_apikey(&mut **connection.as_mut().unwrap(), api_key).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_token(
        &self,
        token: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_set_token(&mut **connection.as_mut().unwrap(), token).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_begin_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_begin_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_commit_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_commit_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_cleanup(&self, table_name: &str) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_cleanup(
            connection.as_mut().unwrap(),
            table_name,
            &self.cloudsync_interrupt,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_terminate(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::terminate(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub(crate) async fn cloudsync_terminate_and_close(&self) -> Result<(), CloudsyncRuntimeError> {
        self.cloudsync_initializer.clear();
        let acquisition_deadline = tokio::time::Instant::now() + CLOUDSYNC_POOL_DRAIN_TIMEOUT;
        let mut pinned =
            tokio::time::timeout_at(acquisition_deadline, self.cloudsync_connection.lock())
                .await
                .map_err(|_| CloudsyncRuntimeError::LocalStatusBusy)?;
        if pinned.is_none() {
            *pinned = Some(
                tokio::time::timeout_at(acquisition_deadline, self.pool.acquire())
                    .await
                    .map_err(|_| CloudsyncRuntimeError::LocalStatusBusy)?
                    .map_err(hypr_cloudsync::Error::from)?,
            );
        }

        let mut connections = Vec::new();
        for _ in 1..self.pool.options().get_max_connections() {
            match tokio::time::timeout_at(acquisition_deadline, self.pool.acquire()).await {
                Ok(Ok(connection)) => connections.push(connection),
                Ok(Err(error)) => {
                    connections.push(pinned.take().unwrap());
                    drop(pinned);
                    if let Err(close_error) = close_pool_connections(connections).await {
                        tracing::warn!(%close_error, "failed to close cloudsync connections after pool acquisition failure");
                    }
                    return Err(hypr_cloudsync::Error::from(error).into());
                }
                Err(_) => {
                    connections.push(pinned.take().unwrap());
                    drop(pinned);
                    if let Err(close_error) = close_pool_connections(connections).await {
                        tracing::warn!(%close_error, "failed to close cloudsync connections after pool acquisition timeout");
                    }
                    return Err(CloudsyncRuntimeError::LocalStatusBusy);
                }
            }
        }

        let mut terminate_error = None;
        if let Err(error) = hypr_cloudsync::terminate(&mut **pinned.as_mut().unwrap()).await {
            terminate_error = Some(error);
        }
        for connection in &mut connections {
            if let Err(error) = hypr_cloudsync::terminate(&mut **connection).await
                && terminate_error.is_none()
            {
                terminate_error = Some(error);
            }
        }

        connections.push(pinned.take().unwrap());
        drop(pinned);
        let close_result = close_pool_connections(connections).await;

        if let Some(error) = terminate_error {
            return Err(error.into());
        }
        close_result.map_err(Into::into)
    }

    pub(crate) async fn cloudsync_close_connection(&self) -> Result<(), hypr_cloudsync::Error> {
        let connection = self.cloudsync_connection.lock().await.take();
        match connection {
            Some(connection) => connection
                .close()
                .await
                .map_err(hypr_cloudsync::Error::from),
            None => Ok(()),
        }
    }

    pub async fn cloudsync_network_cleanup(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_cleanup(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_has_unsent_changes(
        &self,
    ) -> Result<bool, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_has_unsent_changes(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_pending_payload_batch(
        &self,
    ) -> Result<hypr_cloudsync::PendingPayloadBatch, hypr_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_pending_payload_batch(
            connection.as_mut().unwrap(),
            &self.cloudsync_interrupt,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_status(
        &self,
    ) -> Result<hypr_cloudsync::NetworkStatus, hypr_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_network_status(
            connection.as_mut().unwrap(),
            Some(&self.cloudsync_interrupt),
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_reconcile_confirmed_pending_payload(
        &self,
        batch: hypr_cloudsync::PendingPayloadBatch,
        status: &hypr_cloudsync::NetworkStatus,
    ) -> Result<bool, hypr_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::reconcile_confirmed_pending_payload(
            connection.as_mut().unwrap(),
            batch,
            status,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_send_changes(
        &self,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = guarded_interruptible_network_send_changes(
            connection.as_mut().unwrap(),
            &self.cloudsync_interrupt,
            || false,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_receive_changes(
        &self,
        max_chunks: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        // Keep the argument for mobile ABI compatibility while preventing callers
        // from bypassing the one-chunk production receive bound.
        let _ = max_chunks;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_network_receive_changes(
            connection.as_mut().unwrap(),
            &self.cloudsync_interrupt,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_check_changes(
        &self,
        max_chunks: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        self.cloudsync_network_receive_changes(max_chunks).await
    }

    pub async fn cloudsync_network_reset_sync_version(&self) -> Result<(), hypr_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_reset_sync_version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        if result.is_ok() {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_sync = None;
            runtime.last_sync_at_ms = None;
        }
        result
    }

    pub async fn cloudsync_network_reset_receive_version(
        &self,
    ) -> Result<(), hypr_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_reset_receive_version(connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        if result.is_ok() {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_sync = None;
            runtime.last_sync_at_ms = None;
        }
        result
    }

    pub async fn cloudsync_network_logout(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            interruptible_network_logout(connection.as_mut().unwrap(), &self.cloudsync_interrupt)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        // These legacy arguments remain in the mobile ABI; retries now belong to
        // the runtime so every call performs exactly one bounded transport step.
        let _ = (wait_ms, max_retries);
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = async {
            let connection = &mut **connection.as_mut().unwrap();
            let send = guarded_interruptible_network_send_changes(
                &mut *connection,
                &self.cloudsync_interrupt,
                || super::runtime::cloudsync_activity_paused(&self.cloudsync_sync_hook),
            )
            .await?;
            let receive =
                interruptible_network_receive_changes(&mut *connection, &self.cloudsync_interrupt)
                    .await?;
            Ok(merge_bounded_sync_results(send, receive))
        }
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_manual_send_only(
        &self,
        cancelled: &AtomicBool,
    ) -> Result<hypr_cloudsync::NetworkResult, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.ensure_manual_transport_ready()?;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = guarded_interruptible_network_send_changes(
            connection.as_mut().unwrap(),
            &self.cloudsync_interrupt,
            || {
                cancelled.load(Ordering::Acquire)
                    || super::runtime::cloudsync_activity_paused(&self.cloudsync_sync_hook)
            },
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        Ok(result?)
    }

    pub async fn cloudsync_manual_pending_payload_batch(
        &self,
    ) -> Result<hypr_cloudsync::PendingPayloadBatch, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.ensure_manual_transport_ready()?;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_pending_payload_batch(
            connection.as_mut().unwrap(),
            &self.cloudsync_interrupt,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        Ok(result?)
    }

    pub async fn cloudsync_manual_network_status(
        &self,
    ) -> Result<hypr_cloudsync::NetworkStatus, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.ensure_manual_transport_ready()?;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_network_status(
            connection.as_mut().unwrap(),
            Some(&self.cloudsync_interrupt),
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        Ok(result?)
    }

    pub async fn cloudsync_manual_reconcile_confirmed_pending_payload(
        &self,
        batch: hypr_cloudsync::PendingPayloadBatch,
        status: &hypr_cloudsync::NetworkStatus,
    ) -> Result<bool, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.ensure_manual_transport_ready()?;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::reconcile_confirmed_pending_payload(
            connection.as_mut().unwrap(),
            batch,
            status,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        Ok(result?)
    }

    pub async fn cloudsync_manual_receive_one(
        &self,
    ) -> Result<hypr_cloudsync::NetworkResult, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.ensure_manual_transport_ready()?;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = interruptible_network_receive_changes(
            connection.as_mut().unwrap(),
            &self.cloudsync_interrupt,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        Ok(result?)
    }

    pub(crate) async fn apply_cloudsync_auth(
        &self,
        auth: &CloudsyncAuth,
    ) -> Result<(), hypr_cloudsync::Error> {
        match auth {
            CloudsyncAuth::None => Ok(()),
            CloudsyncAuth::ApiKey { api_key } => self.cloudsync_network_set_apikey(api_key).await,
            CloudsyncAuth::Token { token } => self.cloudsync_network_set_token(token).await,
        }
    }

    fn ensure_manual_transport_ready(&self) -> Result<(), CloudsyncRuntimeError> {
        let runtime = self.cloudsync_runtime.lock().unwrap();
        if runtime.running {
            return Err(CloudsyncRuntimeError::RestartRequired);
        }
        if !runtime.network_initialized {
            return Err(CloudsyncRuntimeError::NotStarted);
        }
        Ok(())
    }
}

pub(crate) async fn guarded_interruptible_network_send_changes<F>(
    connection: &mut SqliteConnection,
    interrupt: &super::CloudsyncInterruptHandle,
    cancelled: F,
) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error>
where
    F: Fn() -> bool + Sync,
{
    guarded_network_send_changes_with_interrupt(connection, interrupt, &cancelled).await
}

pub(crate) async fn interruptible_network_receive_changes(
    connection: &mut SqliteConnection,
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
    let registration = interrupt.register(connection).await?;
    let result = hypr_cloudsync::network_receive_changes(&mut *connection, Some(1)).await;
    registration.finish(connection).await?;
    result
}

pub(crate) async fn interruptible_network_logout(
    connection: &mut SqliteConnection,
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<(), hypr_cloudsync::Error> {
    let registration = interrupt.register(connection).await?;
    let result = hypr_cloudsync::network_logout(&mut *connection).await;
    registration.finish(connection).await?;
    result
}

pub(crate) async fn interruptible_cleanup(
    connection: &mut SqliteConnection,
    table_name: &str,
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<(), hypr_cloudsync::Error> {
    let registration = interrupt.register(connection).await?;
    let result = hypr_cloudsync::cleanup(&mut *connection, table_name).await;
    registration.finish(connection).await?;
    result
}

pub(crate) async fn interruptible_init(
    connection: &mut SqliteConnection,
    table_name: &str,
    crdt_algo: Option<&str>,
    init_flags: Option<i64>,
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<(), hypr_cloudsync::Error> {
    let registration = interrupt.register(connection).await?;
    let result = hypr_cloudsync::init(&mut *connection, table_name, crdt_algo, init_flags).await;
    registration.finish(connection).await?;
    result
}

async fn guarded_network_send_changes_with_interrupt(
    connection: &mut SqliteConnection,
    interrupt: &super::CloudsyncInterruptHandle,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
    let batch = ensure_pending_payload_fits(connection, interrupt).await?;
    match interruptible_network_send_changes(connection, Some(interrupt)).await {
        Ok(result) => Ok(result),
        Err(send_error) if should_reconcile_send_failure(batch, &send_error, cancelled()) => {
            let status = match interruptible_network_status(connection, Some(interrupt)).await {
                Ok(status) => status,
                Err(_) => return Err(send_error),
            };
            match hypr_cloudsync::reconcile_confirmed_pending_payload(connection, batch, &status)
                .await
            {
                Ok(true) => Ok(reconciled_send_result(batch, &status)),
                Ok(false) => Err(send_error),
                Err(reconcile_error) => Err(reconcile_error),
            }
        }
        Err(error) => Err(error),
    }
}

fn should_reconcile_send_failure(
    batch: hypr_cloudsync::PendingPayloadBatch,
    error: &hypr_cloudsync::Error,
    cancelled: bool,
) -> bool {
    !cancelled && batch.chunks > 0 && error.kind() == hypr_cloudsync::ErrorKind::Transient
}

async fn interruptible_network_send_changes(
    connection: &mut SqliteConnection,
    interrupt: Option<&super::CloudsyncInterruptHandle>,
) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
    let Some(interrupt) = interrupt else {
        return hypr_cloudsync::network_send_changes(connection).await;
    };
    let registration = interrupt.register(connection).await?;
    let result = hypr_cloudsync::network_send_changes(&mut *connection).await;
    registration.finish(connection).await?;
    result
}

async fn interruptible_network_status(
    connection: &mut SqliteConnection,
    interrupt: Option<&super::CloudsyncInterruptHandle>,
) -> Result<hypr_cloudsync::NetworkStatus, hypr_cloudsync::Error> {
    let Some(interrupt) = interrupt else {
        return hypr_cloudsync::network_status(connection).await;
    };
    let registration = interrupt.register(connection).await?;
    let result = hypr_cloudsync::network_status(&mut *connection).await;
    registration.finish(connection).await?;
    result
}

fn reconciled_send_result(
    batch: hypr_cloudsync::PendingPayloadBatch,
    status: &hypr_cloudsync::NetworkStatus,
) -> hypr_cloudsync::NetworkResult {
    hypr_cloudsync::NetworkResult {
        send: Some(hypr_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: batch.watermark_db_version.unwrap_or(batch.start_db_version),
            server_version: status.last_confirmed_version,
            chunks: i64::from(batch.chunks),
            bytes: i64::try_from(batch.bytes).unwrap_or(i64::MAX),
            last_failure: None,
        }),
        receive: None,
    }
}

fn merge_bounded_sync_results(
    send: hypr_cloudsync::NetworkResult,
    receive: hypr_cloudsync::NetworkResult,
) -> hypr_cloudsync::NetworkResult {
    hypr_cloudsync::NetworkResult {
        send: send.send.or(receive.send),
        receive: receive.receive.or(send.receive),
    }
}

pub(crate) async fn ensure_pending_payload_fits(
    connection: &mut SqliteConnection,
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<hypr_cloudsync::PendingPayloadBatch, hypr_cloudsync::Error> {
    let batch = interruptible_pending_payload_batch(connection, interrupt).await?;
    if !batch.fits {
        return Err(hypr_cloudsync::Error::OutboundPayloadTooLarge {
            chunks: batch.chunks,
            rows: batch.rows,
            bytes: batch.bytes,
            max_chunks: CLOUDSYNC_MAX_OUTBOUND_CHUNKS,
            max_rows: CLOUDSYNC_MAX_OUTBOUND_ROWS,
            max_bytes: CLOUDSYNC_MAX_OUTBOUND_BYTES,
        });
    }
    Ok(batch)
}

async fn interruptible_pending_payload_batch(
    connection: &mut SqliteConnection,
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<hypr_cloudsync::PendingPayloadBatch, hypr_cloudsync::Error> {
    let registration = interrupt.register(connection).await?;
    let result = pending_payload_batch_on(connection).await;
    registration.finish(connection).await?;
    result
}

async fn pending_payload_batch_on(
    connection: &mut SqliteConnection,
) -> Result<hypr_cloudsync::PendingPayloadBatch, hypr_cloudsync::Error> {
    hypr_cloudsync::pending_payload_batch(
        connection,
        CLOUDSYNC_MAX_OUTBOUND_CHUNKS,
        CLOUDSYNC_MAX_OUTBOUND_ROWS,
        CLOUDSYNC_MAX_OUTBOUND_BYTES,
    )
    .await
}

async fn init_enabled_tables(
    connection: &mut SqliteConnection,
    tables: &[CloudsyncTableSpec],
    interrupt: &super::CloudsyncInterruptHandle,
) -> Result<(), hypr_cloudsync::Error> {
    for table in tables.iter().filter(|table| table.enabled) {
        interruptible_init(
            &mut *connection,
            &table.table_name,
            table.crdt_algo.as_deref(),
            table.init_flags,
            interrupt,
        )
        .await?;
    }

    Ok(())
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

pub async fn cloudsync_is_enabled_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<bool, hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::is_enabled(executor, table_name).await
}

pub(crate) async fn cloudsync_has_local_unsent_changes_on<'e, E>(
    executor: E,
) -> Result<bool, hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1
            FROM cloudsync_changes
            WHERE site_id = (
                SELECT site_id
                FROM cloudsync_site_id
                WHERE rowid = 0
            )
              AND db_version > COALESCE(
                (
                    SELECT CAST(value AS INTEGER)
                    FROM cloudsync_settings
                    WHERE key = 'send_dbversion'
                ),
                0
              )
            LIMIT 1
        )",
    )
    .fetch_one(executor)
    .await?)
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

async fn close_pool_connections(
    connections: Vec<PoolConnection<Sqlite>>,
) -> Result<(), hypr_cloudsync::Error> {
    let mut first_error = None;
    for connection in connections {
        if let Err(error) = connection.close().await
            && first_error.is_none()
        {
            first_error = Some(error.into());
        }
    }

    first_error.map_or(Ok(()), Err)
}

async fn return_pool_connections(mut connections: Vec<PoolConnection<Sqlite>>) {
    for connection in &mut connections {
        connection.return_to_pool().await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    #[cfg(target_os = "macos")]
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    async fn db_with_oversized_pending_payload() -> Db {
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
        sqlx::query("SELECT cloudsync_set('payload_max_chunk_size', '262144')")
            .fetch_optional(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO items (id, value) VALUES (?, ?)")
            .bind("large-value")
            .bind("x".repeat(3 * 1024 * 1024))
            .execute(db.pool())
            .await
            .unwrap();
        db
    }

    fn assert_outbound_payload_too_large(error: hypr_cloudsync::Error) {
        assert!(matches!(
            error,
            hypr_cloudsync::Error::OutboundPayloadTooLarge {
                chunks,
                max_chunks: CLOUDSYNC_MAX_OUTBOUND_CHUNKS,
                ..
            } if chunks == CLOUDSYNC_MAX_OUTBOUND_CHUNKS + 1
        ));
    }

    #[cfg(target_os = "macos")]
    #[derive(Clone, Copy, Debug)]
    enum StalledNetworkOperation {
        ManualSend,
        ManualReceive,
        LegacySync,
    }

    #[cfg(target_os = "macos")]
    async fn assert_stalled_operation_drains_before_local_write(
        operation: StalledNetworkOperation,
    ) {
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
        if matches!(
            operation,
            StalledNetworkOperation::ManualSend | StalledNetworkOperation::LegacySync
        ) {
            sqlx::query("INSERT INTO items (id, value) VALUES ('before', 'pending')")
                .execute(db.pool())
                .await
                .unwrap();
        }
        {
            let mut connection = db.cloudsync_connection.lock().await;
            *connection = Some(db.pool.acquire().await.unwrap());
            sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
                .bind(endpoint)
                .bind("manual-interrupt-test")
                .fetch_optional(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }
        db.cloudsync_runtime.lock().unwrap().network_initialized = true;

        let cancelled = Arc::new(AtomicBool::new(false));
        let request_db = Arc::clone(&db);
        let request_cancelled = Arc::clone(&cancelled);
        let request = tokio::spawn(async move {
            match operation {
                StalledNetworkOperation::ManualSend => request_db
                    .cloudsync_manual_send_only(request_cancelled.as_ref())
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                StalledNetworkOperation::ManualReceive => request_db
                    .cloudsync_manual_receive_one()
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                StalledNetworkOperation::LegacySync => request_db
                    .cloudsync_network_sync(None, None)
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
            }
        });
        tokio::task::spawn_blocking(move || {
            accepted_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| {
                    panic!("{operation:?} CloudSync request did not reach the blackhole server")
                });
        })
        .await
        .unwrap();

        cancelled.store(true, Ordering::Release);
        let interrupted_at = std::time::Instant::now();
        while !request.is_finished() {
            db.cloudsync_interrupt_sync();
            assert!(
                interrupted_at.elapsed() < Duration::from_secs(2),
                "manual CloudSync request did not honor sqlite3_interrupt"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(
            request.await.unwrap().is_err(),
            "interrupted manual request succeeded"
        );
        tokio::time::timeout(
            Duration::from_millis(250),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect("manual CloudSync worker did not become idle after interruption");
        db.cloudsync_connection.lock().await.take();
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
                .execute(db.pool()),
        )
        .await
        .expect("immediate local write remained blocked after manual CloudSync interruption")
        .unwrap();

        let _ = release_tx.send(());
        server.join().unwrap();
        db.cloudsync_close_connection().await.unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn manual_and_legacy_network_calls_drain_before_local_writes() {
        for _ in 0..3 {
            assert_stalled_operation_drains_before_local_write(StalledNetworkOperation::ManualSend)
                .await;
            assert_stalled_operation_drains_before_local_write(
                StalledNetworkOperation::ManualReceive,
            )
            .await;
        }
        assert_stalled_operation_drains_before_local_write(StalledNetworkOperation::LegacySync)
            .await;
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn native_logout_cleanup_interrupt_drains_before_local_write() {
        let directory = tempfile::tempdir().unwrap();
        let db = Arc::new(
            Db::connect_local(directory.path().join("logout-interrupt.db"))
                .await
                .unwrap(),
        );
        sqlx::query(
            "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
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
        db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
            table_name: "items".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }])
        .await
        .unwrap();
        let request_db = Arc::clone(&db);
        let request = tokio::spawn(async move { request_db.cloudsync_network_logout().await });
        let interrupted_at = std::time::Instant::now();
        let mut registered = false;
        while !request.is_finished() {
            registered |= db.cloudsync_interrupt_sync();
            assert!(
                interrupted_at.elapsed() < Duration::from_secs(2),
                "native CloudSync logout did not honor sqlite3_interrupt"
            );
            tokio::task::yield_now().await;
        }
        assert!(
            registered,
            "native CloudSync logout completed before its interrupt registration was observable"
        );
        assert!(
            request.await.unwrap().is_err(),
            "interrupted native CloudSync logout succeeded"
        );
        tokio::time::timeout(
            Duration::from_millis(250),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect("native CloudSync logout worker did not become idle after interruption");
        db.cloudsync_version()
            .await
            .expect("native CloudSync logout interruption crashed the pinned SQLite worker");
        db.cloudsync_terminate_and_close().await.unwrap();
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
                .execute(db.pool()),
        )
        .await
        .expect("immediate local write remained blocked after native CloudSync logout interruption")
        .unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn tracked_table_cleanup_interrupt_drains_before_local_write() {
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

        let request_db = Arc::clone(&db);
        let request = tokio::spawn(async move { request_db.cloudsync_cleanup("items").await });
        let interrupted_at = std::time::Instant::now();
        let mut registered = false;
        while !request.is_finished() {
            registered |= db.cloudsync_interrupt_sync();
            assert!(
                interrupted_at.elapsed() < Duration::from_secs(2),
                "native CloudSync table cleanup did not honor sqlite3_interrupt"
            );
            tokio::task::yield_now().await;
        }
        assert!(
            registered,
            "native CloudSync table cleanup completed before its interrupt registration was observable"
        );
        assert!(
            request.await.unwrap().is_err(),
            "interrupted native CloudSync table cleanup succeeded"
        );
        tokio::time::timeout(
            Duration::from_millis(250),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect("native CloudSync table cleanup worker did not become idle after interruption");
        db.cloudsync_version()
            .await
            .expect("native CloudSync table cleanup interruption crashed the pinned SQLite worker");
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
                .execute(db.pool()),
        )
        .await
        .expect(
            "immediate local write remained blocked after native CloudSync table cleanup interruption",
        )
        .unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn populated_table_init_interrupt_drains_before_local_write() {
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

        let request_db = Arc::clone(&db);
        let request = tokio::spawn(async move {
            request_db
                .cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
                    table_name: "items".to_string(),
                    crdt_algo: None,
                    init_flags: None,
                    enabled: true,
                }])
                .await
        });
        let interrupted_at = std::time::Instant::now();
        let mut registered = false;
        while !request.is_finished() {
            registered |= db.cloudsync_interrupt_sync();
            assert!(
                interrupted_at.elapsed() < Duration::from_secs(2),
                "native CloudSync table initialization did not honor sqlite3_interrupt"
            );
            tokio::task::yield_now().await;
        }
        assert!(
            registered,
            "native CloudSync table initialization completed before its interrupt registration was observable"
        );
        assert!(
            request.await.unwrap().is_err(),
            "interrupted native CloudSync table initialization succeeded"
        );
        tokio::time::timeout(
            Duration::from_millis(250),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect(
            "native CloudSync table initialization worker did not become idle after interruption",
        );
        db.cloudsync_version().await.expect(
            "native CloudSync table initialization interruption crashed the pinned SQLite worker",
        );
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
                .execute(db.pool()),
        )
        .await
        .expect(
            "immediate local write remained blocked after native CloudSync table initialization interruption",
        )
        .unwrap();
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn pending_payload_interrupt_drains_before_local_write() {
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

        let request_db = Arc::clone(&db);
        let request =
            tokio::spawn(async move { request_db.cloudsync_pending_payload_batch().await });
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
        let interrupted_at = std::time::Instant::now();
        while !request.is_finished() {
            assert!(db.cloudsync_interrupt_sync());
            assert!(
                interrupted_at.elapsed() < Duration::from_secs(2),
                "pending payload generation did not honor sqlite3_interrupt"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(
            request.await.unwrap().is_err(),
            "interrupted pending payload generation succeeded"
        );
        tokio::time::timeout(
            Duration::from_millis(250),
            db.cloudsync_wait_for_sync_idle(),
        )
        .await
        .expect("pending payload worker did not become idle after interruption");
        db.cloudsync_connection.lock().await.take();
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO items (id, value) VALUES ('after', 'local')")
                .execute(db.pool()),
        )
        .await
        .expect("immediate local write remained blocked after pending payload interruption")
        .unwrap();
    }

    #[test]
    fn reconciled_send_reports_the_exact_preflighted_batch() {
        let batch = hypr_cloudsync::PendingPayloadBatch {
            start_db_version: 4,
            watermark_db_version: Some(9),
            chunks: 2,
            rows: 8,
            bytes: 4096,
            complete: true,
            fits: true,
        };
        let status = hypr_cloudsync::NetworkStatus {
            last_optimistic_version: 12,
            last_confirmed_version: 12,
            gaps: Vec::new(),
            failures: hypr_cloudsync::NetworkStatusFailures::default(),
        };

        let result = reconciled_send_result(batch, &status);
        let send = result.send.unwrap();

        assert_eq!(send.status, "synced");
        assert_eq!(send.local_version, 9);
        assert_eq!(send.server_version, 12);
        assert_eq!(send.chunks, 2);
        assert_eq!(send.bytes, 4096);
    }

    #[test]
    fn cancelled_send_never_starts_status_reconciliation() {
        let batch = hypr_cloudsync::PendingPayloadBatch {
            start_db_version: 4,
            watermark_db_version: Some(9),
            chunks: 2,
            rows: 8,
            bytes: 4096,
            complete: true,
            fits: true,
        };
        let error = hypr_cloudsync::Error::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "interrupted upload",
        ));

        assert!(should_reconcile_send_failure(batch, &error, false));
        assert!(!should_reconcile_send_failure(batch, &error, true));
    }

    #[test]
    fn bounded_sync_preserves_the_send_and_single_receive_results() {
        let send = hypr_cloudsync::NetworkResult {
            send: Some(hypr_cloudsync::NetworkSendResult {
                status: "synced".to_string(),
                local_version: 9,
                server_version: 9,
                chunks: 2,
                bytes: 4096,
                last_failure: None,
            }),
            receive: None,
        };
        let receive = hypr_cloudsync::NetworkResult {
            send: None,
            receive: Some(hypr_cloudsync::NetworkReceiveResult {
                rows: 3,
                tables: vec!["items".to_string()],
                chunks: 1,
                bytes: 2048,
                complete: false,
                error: None,
                last_failure: None,
            }),
        };

        let result = merge_bounded_sync_results(send, receive);

        assert_eq!(result.send.unwrap().chunks, 2);
        let receive = result.receive.unwrap();
        assert_eq!(receive.chunks, 1);
        assert!(!receive.complete);
    }

    #[tokio::test]
    async fn oversized_payload_never_reaches_send_transport() {
        let db = db_with_oversized_pending_payload().await;

        let error = db.cloudsync_network_send_changes().await.unwrap_err();

        assert_outbound_payload_too_large(error);
    }

    #[tokio::test]
    async fn public_network_sync_cannot_bypass_the_send_guard() {
        let db = db_with_oversized_pending_payload().await;

        let error = db.cloudsync_network_sync(None, None).await.unwrap_err();

        assert_outbound_payload_too_large(error);
    }

    #[tokio::test]
    async fn network_calls_reuse_one_checked_out_connection() {
        let db = Arc::new(Db::connect_memory_plain().await.unwrap());
        {
            let mut connection = db.lock_cloudsync_connection().await.unwrap();
            sqlx::query("CREATE TEMP TABLE cloudsync_connection_marker (value INTEGER)")
                .execute(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }

        let mut connection = db.lock_cloudsync_connection().await.unwrap();
        let marker_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'cloudsync_connection_marker'",
        )
        .fetch_one(&mut **connection.as_mut().unwrap())
        .await
        .unwrap();

        assert_eq!(marker_exists, 1);
    }

    #[tokio::test]
    async fn network_sync_waits_for_checked_out_connection() {
        let db = Arc::new(Db::connect_memory_plain().await.unwrap());
        let guard = db.lock_cloudsync_connection().await.unwrap();
        let task_db = Arc::clone(&db);
        let mut task =
            tokio::spawn(async move { task_db.cloudsync_network_sync(None, None).await });

        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut task)
                .await
                .is_err()
        );

        drop(guard);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn initializing_tables_updates_every_pool_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut preexisting_connections = Vec::new();
        for _ in 0..4 {
            preexisting_connections.push(db.pool().acquire().await.unwrap());
        }
        drop(preexisting_connections);

        db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
            table_name: "sessions".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }])
        .await
        .unwrap();

        let mut write_connections = Vec::new();
        for _ in 0..3 {
            write_connections.push(db.pool().acquire().await.unwrap());
        }
        for (index, connection) in write_connections.iter_mut().enumerate() {
            sqlx::query("INSERT INTO sessions (id, title) VALUES (?, 'Note')")
                .bind(format!("session-{index}"))
                .execute(&mut **connection)
                .await
                .unwrap();
        }
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn table_init_bounds_pool_drain_and_reuses_connections_after_retry() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(3),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let tables = [CloudsyncTableSpec {
            table_name: "sessions".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }];
        let mut held_connection = db.pool().acquire().await.unwrap();

        let started_at = std::time::Instant::now();
        let error = tokio::time::timeout(
            Duration::from_millis(1_500),
            db.cloudsync_init_enabled_tables(&tables),
        )
        .await
        .expect("CloudSync table initialization exceeded its pool-drain deadline")
        .unwrap_err();
        assert!(matches!(error, CloudsyncRuntimeError::LocalStatusBusy));
        assert!(started_at.elapsed() < Duration::from_millis(1_500));

        held_connection.return_to_pool().await;
        db.cloudsync_init_enabled_tables(&tables).await.unwrap();

        let mut connections = Vec::new();
        for index in 0..2 {
            let mut connection =
                tokio::time::timeout(Duration::from_millis(250), db.pool().acquire())
                    .await
                    .expect("CloudSync table initialization leaked a pool connection")
                    .unwrap();
            assert!(
                cloudsync_is_enabled_on(&mut *connection, "sessions")
                    .await
                    .unwrap()
            );
            sqlx::query("INSERT INTO sessions (id, title) VALUES (?, 'Note')")
                .bind(format!("session-{index}"))
                .execute(&mut *connection)
                .await
                .unwrap();
            connections.push(connection);
        }
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn table_init_error_closes_partially_initialized_drained_connections() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(3),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut preexisting_connections = Vec::new();
        for _ in 0..3 {
            let mut connection = db.pool().acquire().await.unwrap();
            sqlx::query("CREATE TEMP TABLE init_connection_marker (id INTEGER)")
                .execute(&mut *connection)
                .await
                .unwrap();
            preexisting_connections.push(connection);
        }
        for connection in &mut preexisting_connections {
            connection.return_to_pool().await;
        }

        db.cloudsync_init_enabled_tables(&[
            CloudsyncTableSpec {
                table_name: "sessions".to_string(),
                crdt_algo: None,
                init_flags: None,
                enabled: true,
            },
            CloudsyncTableSpec {
                table_name: "missing_table".to_string(),
                crdt_algo: None,
                init_flags: None,
                enabled: true,
            },
        ])
        .await
        .unwrap_err();

        let mut replacements = Vec::new();
        for _ in 0..2 {
            let mut connection =
                tokio::time::timeout(Duration::from_millis(250), db.pool().acquire())
                    .await
                    .expect("failed initialization leaked a drained pool connection")
                    .unwrap();
            let marker_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*)
                 FROM sqlite_temp_master
                 WHERE name = 'init_connection_marker'",
            )
            .fetch_one(&mut *connection)
            .await
            .unwrap();
            assert_eq!(marker_count, 0);
            replacements.push(connection);
        }
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn receive_version_reset_preserves_send_cursor_on_pinned_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("sessions", None, None).await.unwrap();

        {
            let mut connection = db.lock_cloudsync_connection().await.unwrap();
            sqlx::query("INSERT INTO sessions (id, title) VALUES ('local', 'Pending')")
                .execute(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
            sqlx::query(
                "SELECT
                    cloudsync_set('check_dbversion', '31'),
                    cloudsync_set('check_seq', '7'),
                    cloudsync_set('send_dbversion', '19'),
                    cloudsync_set('send_seq', '4')",
            )
            .fetch_optional(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
        }

        db.cloudsync_network_reset_receive_version().await.unwrap();

        let mut connection = db.lock_cloudsync_connection().await.unwrap();
        let cursors: (String, String, String, String) = sqlx::query_as(
            "SELECT
                MAX(CASE WHEN key = 'check_dbversion' THEN value END),
                MAX(CASE WHEN key = 'check_seq' THEN value END),
                MAX(CASE WHEN key = 'send_dbversion' THEN value END),
                MAX(CASE WHEN key = 'send_seq' THEN value END)
             FROM cloudsync_settings",
        )
        .fetch_one(&mut **connection.as_mut().unwrap())
        .await
        .unwrap();
        assert_eq!(cursors, ("0".into(), "0".into(), "19".into(), "4".into()));
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn terminating_cloudsync_closes_a_single_pool_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("sessions", None, None).await.unwrap();
        {
            let mut connection = db.lock_cloudsync_connection().await.unwrap();
            sqlx::query("CREATE TEMP TABLE connection_marker (id INTEGER)")
                .execute(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }

        db.cloudsync_terminate_and_close().await.unwrap();

        let marker_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'connection_marker'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(marker_count, 0);
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('session', 'Note')")
            .execute(db.pool())
            .await
            .unwrap();
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn terminating_cloudsync_closes_every_pool_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(3),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
            table_name: "sessions".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }])
        .await
        .unwrap();

        let mut connections = Vec::new();
        for _ in 0..2 {
            let mut connection = db.pool().acquire().await.unwrap();
            sqlx::query("CREATE TEMP TABLE connection_marker (id INTEGER)")
                .execute(&mut *connection)
                .await
                .unwrap();
            connections.push(connection);
        }
        drop(connections);

        db.cloudsync_terminate_and_close().await.unwrap();

        let mut replacements = Vec::new();
        for index in 0..3 {
            let mut connection = db.pool().acquire().await.unwrap();
            let marker_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'connection_marker'",
            )
            .fetch_one(&mut *connection)
            .await
            .unwrap();
            assert_eq!(marker_count, 0);
            sqlx::query("INSERT INTO sessions (id, title) VALUES (?, 'Note')")
                .bind(format!("session-{index}"))
                .execute(&mut *connection)
                .await
                .unwrap();
            replacements.push(connection);
        }
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn suspend_bounds_pool_drain_and_allows_later_teardown() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(3),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.cloudsync_init("sessions", None, None).await.unwrap();
        let held_connection = db.pool().acquire().await.unwrap();

        let started_at = std::time::Instant::now();
        let error = tokio::time::timeout(Duration::from_millis(1_500), db.cloudsync_suspend())
            .await
            .expect("CloudSync suspend exceeded its pool-drain deadline")
            .unwrap_err();
        assert!(matches!(error, CloudsyncRuntimeError::LocalStatusBusy));
        assert!(started_at.elapsed() < Duration::from_millis(1_500));

        drop(held_connection);
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO sessions (id, title) VALUES ('after-timeout', 'Note')")
                .execute(db.pool()),
        )
        .await
        .expect("local write could not reuse the pool after the drain timeout")
        .unwrap();

        db.cloudsync_terminate_and_close().await.unwrap();
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO sessions (id, title) VALUES ('after-retry', 'Note')")
                .execute(db.pool()),
        )
        .await
        .expect("local write could not reuse the pool after teardown retry")
        .unwrap();
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn suspend_bounds_initial_pinned_connection_acquisition() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let held_connection = db.pool().acquire().await.unwrap();

        let error = tokio::time::timeout(Duration::from_millis(1_500), db.cloudsync_suspend())
            .await
            .expect("CloudSync suspend exceeded its initial pool-acquisition deadline")
            .unwrap_err();
        assert!(matches!(error, CloudsyncRuntimeError::LocalStatusBusy));

        drop(held_connection);
        db.cloudsync_suspend().await.unwrap();
        tokio::time::timeout(
            Duration::from_millis(250),
            sqlx::query("INSERT INTO sessions (id, title) VALUES ('after-retry', 'Note')")
                .execute(db.pool()),
        )
        .await
        .expect("local write could not reuse the pool after initial acquisition retry")
        .unwrap();
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    #[tokio::test]
    async fn initializes_replacement_pool_connections() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cloudsync.db");
        let db = Db::open(crate::DbOpenOptions {
            storage: crate::DbStorage::Local(&db_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();

        db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
            table_name: "sessions".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }])
        .await
        .unwrap();

        let connection = db.pool().acquire().await.unwrap();
        connection.close().await.unwrap();
        let mut replacement = db.pool().acquire().await.unwrap();

        sqlx::query("INSERT INTO sessions (id, title) VALUES ('replacement', 'Note')")
            .execute(&mut *replacement)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn closes_pinned_connection_without_cloudsync_extension() {
        let db = Db::connect_local_plain(tempfile::NamedTempFile::new().unwrap().path())
            .await
            .unwrap();
        drop(db.lock_cloudsync_connection().await.unwrap());
        assert!(db.cloudsync_connection.lock().await.is_some());

        db.cloudsync_close_connection().await.unwrap();

        assert!(db.cloudsync_connection.lock().await.is_none());
    }
}
