use anlg_cloudsync::{OwnedSqliteConnection, ReservedConnection};

use super::super::CloudsyncInterruptHandle;
use super::payload::ensure_pending_payload_fits;
use super::schema::cloudsync_has_local_unsent_changes_on;

pub(crate) async fn guarded_interruptible_network_send_changes<C, F>(
    connection: &mut ReservedConnection<C>,
    interrupt: &CloudsyncInterruptHandle,
    cancelled: F,
) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error>
where
    C: OwnedSqliteConnection,
    F: Fn() -> bool + Sync,
{
    guarded_network_send_changes_with_interrupt(connection, interrupt, &cancelled).await
}

pub(crate) async fn interruptible_network_receive_changes<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    interrupt: &CloudsyncInterruptHandle,
) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
    let registration = interrupt.register(connection.connection().await?).await?;
    let result = anlg_cloudsync::network_receive_changes_on_connection(connection, Some(1)).await;
    registration.finish(connection.connection().await?).await?;
    result
}

pub(crate) async fn interruptible_network_logout<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    interrupt: &CloudsyncInterruptHandle,
) -> Result<(), anlg_cloudsync::Error> {
    let registration = interrupt.register(connection.connection().await?).await?;
    let result = anlg_cloudsync::network_logout_on_connection(connection).await;
    registration.finish(connection.connection().await?).await?;
    result
}

async fn guarded_network_send_changes_with_interrupt<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    interrupt: &CloudsyncInterruptHandle,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
    let batch = ensure_pending_payload_fits(connection, interrupt).await?;
    match interruptible_network_send_changes(
        connection,
        interrupt,
        // Upstream rejects a zero bound; an empty batch sends nothing anyway,
        // and a lone late write stays bounded to one version.
        batch.local_db_versions.max(1),
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(send_error) if should_reconcile_send_failure(batch, &send_error, cancelled()) => {
            let status = match interruptible_network_status(connection, Some(interrupt)).await {
                Ok(status) => status,
                Err(status_error) => {
                    tracing::warn!(
                        start_db_version = batch.start_db_version,
                        watermark_db_version = ?batch.watermark_db_version,
                        status_error_kind = ?status_error.kind(),
                        "CloudSync send reconciliation status unavailable"
                    );
                    return Err(send_error);
                }
            };
            match anlg_cloudsync::reconcile_confirmed_pending_payload(
                connection.connection().await?,
                batch,
                &status,
            )
            .await
            {
                Ok(true) => {
                    let has_unsent_changes =
                        cloudsync_has_local_unsent_changes_on(connection.connection().await?)
                            .await?;
                    Ok(reconciled_send_result(batch, &status, has_unsent_changes))
                }
                Ok(false) => {
                    tracing::warn!(
                        start_db_version = batch.start_db_version,
                        watermark_db_version = ?batch.watermark_db_version,
                        chunks = batch.chunks,
                        rows = batch.rows,
                        bytes = batch.bytes,
                        complete = batch.complete,
                        fits = batch.fits,
                        last_optimistic_version = status.last_optimistic_version,
                        last_confirmed_version = status.last_confirmed_version,
                        gap_count = status.gaps.len(),
                        apply_failure = status.failures.apply.is_some(),
                        "CloudSync pending send could not be reconciled"
                    );
                    Err(send_error)
                }
                Err(reconcile_error) => Err(reconcile_error),
            }
        }
        Err(error) => Err(error),
    }
}

pub(super) fn should_reconcile_send_failure(
    batch: anlg_cloudsync::PendingPayloadBatch,
    error: &anlg_cloudsync::Error,
    cancelled: bool,
) -> bool {
    !cancelled && batch.chunks > 0 && error.kind() == anlg_cloudsync::ErrorKind::Transient
}

async fn interruptible_network_send_changes<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    interrupt: &CloudsyncInterruptHandle,
    max_db_versions: i64,
) -> Result<anlg_cloudsync::NetworkResult, anlg_cloudsync::Error> {
    let registration = interrupt.register(connection.connection().await?).await?;
    let result =
        anlg_cloudsync::network_send_changes_bounded_on_connection(connection, max_db_versions)
            .await;
    registration.finish(connection.connection().await?).await?;
    result
}

pub(super) async fn interruptible_network_status<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    interrupt: Option<&CloudsyncInterruptHandle>,
) -> Result<anlg_cloudsync::NetworkStatus, anlg_cloudsync::Error> {
    let Some(interrupt) = interrupt else {
        return anlg_cloudsync::network_status(connection.connection().await?).await;
    };
    let registration = interrupt.register(connection.connection().await?).await?;
    let result = anlg_cloudsync::network_status_on_connection(connection).await;
    registration.finish(connection.connection().await?).await?;
    result
}

pub(super) fn reconciled_send_result(
    batch: anlg_cloudsync::PendingPayloadBatch,
    status: &anlg_cloudsync::NetworkStatus,
    has_unsent_changes: bool,
) -> anlg_cloudsync::NetworkResult {
    let fully_confirmed = batch.watermark_db_version.is_some_and(|watermark| {
        status.last_optimistic_version >= watermark && status.last_confirmed_version >= watermark
    });
    anlg_cloudsync::NetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: if !fully_confirmed || batch.remaining || has_unsent_changes {
                "syncing"
            } else {
                "synced"
            }
            .to_string(),
            local_version: batch.watermark_db_version.unwrap_or(batch.start_db_version),
            server_version: status.last_confirmed_version,
            chunks: if fully_confirmed {
                i64::from(batch.chunks)
            } else {
                0
            },
            bytes: if fully_confirmed {
                i64::try_from(batch.bytes).unwrap_or(i64::MAX)
            } else {
                0
            },
            last_failure: None,
        }),
        receive: None,
    }
}

pub(super) fn merge_bounded_sync_results(
    send: anlg_cloudsync::NetworkResult,
    receive: anlg_cloudsync::NetworkResult,
) -> anlg_cloudsync::NetworkResult {
    anlg_cloudsync::NetworkResult {
        send: send.send.or(receive.send),
        receive: receive.receive.or(send.receive),
    }
}
