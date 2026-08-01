use sqlx::Sqlite;
use sqlx::pool::PoolConnection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::MutexGuard;

use super::{CloudsyncAuth, CloudsyncRuntimeError, CloudsyncTableSpec};
use crate::Db;

mod payload;
mod pool;
mod schema;
mod transport;

pub(crate) use payload::ensure_pending_payload_fits;
use payload::interruptible_pending_payload_batch;
pub use payload::{
    CLOUDSYNC_MAX_OUTBOUND_BYTES, CLOUDSYNC_MAX_OUTBOUND_CHUNKS, CLOUDSYNC_MAX_OUTBOUND_ROWS,
};
use pool::{close_pool_connections, return_pool_connections};
pub(crate) use schema::cloudsync_has_local_unsent_changes_on;
pub use schema::{cloudsync_begin_alter_on, cloudsync_commit_alter_on, cloudsync_is_enabled_on};
use schema::{init_enabled_tables, interruptible_cleanup, interruptible_init};
pub(crate) use transport::{
    guarded_interruptible_network_send_changes, interruptible_network_receive_changes,
};
use transport::{
    interruptible_network_logout, interruptible_network_status, merge_bounded_sync_results,
};
#[cfg(test)]
use transport::{reconciled_send_result, should_reconcile_send_failure};

const CLOUDSYNC_POOL_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

impl Db {
    async fn lock_cloudsync_connection(
        &self,
    ) -> Result<MutexGuard<'_, Option<PoolConnection<Sqlite>>>, anlg_cloudsync::Error> {
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

    pub async fn cloudsync_version(&self) -> Result<String, anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = anlg_cloudsync::version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        init_flags: Option<i64>,
    ) -> Result<(), anlg_cloudsync::Error> {
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
    ) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = anlg_cloudsync::set_filter(
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
                    .map_err(anlg_cloudsync::Error::from)?,
            );
        }

        let mut connections = Vec::new();
        for _ in 1..self.pool.options().get_max_connections() {
            match tokio::time::timeout_at(acquisition_deadline, self.pool.acquire()).await {
                Ok(Ok(connection)) => connections.push(connection),
                Ok(Err(error)) => {
                    return_pool_connections(connections).await;
                    self.release_single_pool_connection(&mut pinned);
                    return Err(anlg_cloudsync::Error::from(error).into());
                }
                Err(_) => {
                    return_pool_connections(connections).await;
                    self.release_single_pool_connection(&mut pinned);
                    return Err(CloudsyncRuntimeError::LocalStatusBusy);
                }
            }
        }

        let result: Result<(), anlg_cloudsync::Error> = async {
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
    ) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            anlg_cloudsync::network_init(&mut **connection.as_mut().unwrap(), connection_string)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_apikey(
        &self,
        api_key: &str,
    ) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            anlg_cloudsync::network_set_apikey(&mut **connection.as_mut().unwrap(), api_key).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_token(
        &self,
        token: &str,
    ) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            anlg_cloudsync::network_set_token(&mut **connection.as_mut().unwrap(), token).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_begin_alter(
        &self,
        table_name: &str,
    ) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_begin_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_commit_alter(
        &self,
        table_name: &str,
    ) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_commit_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_cleanup(&self, table_name: &str) -> Result<(), anlg_cloudsync::Error> {
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

    pub async fn cloudsync_terminate(&self) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = anlg_cloudsync::terminate(&mut **connection.as_mut().unwrap()).await;
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
                    .map_err(anlg_cloudsync::Error::from)?,
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
                    return Err(anlg_cloudsync::Error::from(error).into());
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
        if let Err(error) = anlg_cloudsync::terminate(&mut **pinned.as_mut().unwrap()).await {
            terminate_error = Some(error);
        }
        for connection in &mut connections {
            if let Err(error) = anlg_cloudsync::terminate(&mut **connection).await
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

    pub(crate) async fn cloudsync_close_connection(&self) -> Result<(), anlg_cloudsync::Error> {
        let connection = self.cloudsync_connection.lock().await.take();
        match connection {
            Some(connection) => connection
                .close()
                .await
                .map_err(anlg_cloudsync::Error::from),
            None => Ok(()),
        }
    }

    pub async fn cloudsync_network_cleanup(&self) -> Result<(), anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = anlg_cloudsync::network_cleanup(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_has_unsent_changes(
        &self,
    ) -> Result<bool, anlg_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            anlg_cloudsync::network_has_unsent_changes(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_pending_payload_batch(
        &self,
    ) -> Result<anlg_cloudsync::PendingPayloadBatch, anlg_cloudsync::Error> {
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
    ) -> Result<anlg_cloudsync::NetworkStatus, anlg_cloudsync::Error> {
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
        batch: anlg_cloudsync::PendingPayloadBatch,
        status: &anlg_cloudsync::NetworkStatus,
    ) -> Result<bool, anlg_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = anlg_cloudsync::reconcile_confirmed_pending_payload(
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
    ) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
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
    ) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
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
    ) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
        self.cloudsync_network_receive_changes(max_chunks).await
    }

    pub async fn cloudsync_network_reset_sync_version(&self) -> Result<(), anlg_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            anlg_cloudsync::network_reset_sync_version(&mut **connection.as_mut().unwrap()).await;
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
    ) -> Result<(), anlg_cloudsync::Error> {
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            anlg_cloudsync::network_reset_receive_version(connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        if result.is_ok() {
            let mut runtime = self.cloudsync_runtime.lock().unwrap();
            runtime.last_sync = None;
            runtime.last_sync_at_ms = None;
        }
        result
    }

    pub async fn cloudsync_network_logout(&self) -> Result<(), anlg_cloudsync::Error> {
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
    ) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
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
    ) -> Result<anlg_cloudsync::NetworkResult, CloudsyncRuntimeError> {
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
    ) -> Result<anlg_cloudsync::PendingPayloadBatch, CloudsyncRuntimeError> {
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
    ) -> Result<anlg_cloudsync::NetworkStatus, CloudsyncRuntimeError> {
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
        batch: anlg_cloudsync::PendingPayloadBatch,
        status: &anlg_cloudsync::NetworkStatus,
    ) -> Result<bool, CloudsyncRuntimeError> {
        let _lifecycle = self.cloudsync_lifecycle.lock().await;
        self.ensure_manual_transport_ready()?;
        let _sync_operation = self.cloudsync_sync_operation.lock().await;
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = anlg_cloudsync::reconcile_confirmed_pending_payload(
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
    ) -> Result<anlg_cloudsync::NetworkResult, CloudsyncRuntimeError> {
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
    ) -> Result<(), anlg_cloudsync::Error> {
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

#[cfg(test)]
mod tests;
