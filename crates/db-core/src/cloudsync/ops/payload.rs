use sqlx::SqliteConnection;

use super::super::CloudsyncInterruptHandle;

pub const CLOUDSYNC_MAX_OUTBOUND_CHUNKS: u32 = 8;
pub const CLOUDSYNC_MAX_OUTBOUND_ROWS: u64 = 4_096;
pub const CLOUDSYNC_MAX_OUTBOUND_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) async fn ensure_pending_payload_fits(
    connection: &mut SqliteConnection,
    interrupt: &CloudsyncInterruptHandle,
) -> Result<anlg_cloudsync::PendingPayloadBatch, anlg_cloudsync::Error> {
    let batch = interruptible_pending_payload_batch(connection, interrupt).await?;
    if !batch.fits {
        return Err(anlg_cloudsync::Error::OutboundPayloadTooLarge {
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

pub(super) async fn interruptible_pending_payload_batch(
    connection: &mut SqliteConnection,
    interrupt: &CloudsyncInterruptHandle,
) -> Result<anlg_cloudsync::PendingPayloadBatch, anlg_cloudsync::Error> {
    let registration = interrupt.register(connection).await?;
    let result = pending_payload_batch_on(connection).await;
    registration.finish(connection).await?;
    result
}

async fn pending_payload_batch_on(
    connection: &mut SqliteConnection,
) -> Result<anlg_cloudsync::PendingPayloadBatch, anlg_cloudsync::Error> {
    anlg_cloudsync::pending_payload_batch(
        connection,
        CLOUDSYNC_MAX_OUTBOUND_CHUNKS,
        CLOUDSYNC_MAX_OUTBOUND_ROWS,
        CLOUDSYNC_MAX_OUTBOUND_BYTES,
    )
    .await
}
