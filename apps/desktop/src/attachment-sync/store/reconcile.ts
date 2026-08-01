import {
  ACTIVE_PHASES,
  type AttachmentTransferDirection,
  type ObsoleteDownloadJobRow,
  type ReconcileRow,
} from "./types";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

export async function reconcileAttachmentTransferJobs(): Promise<number> {
  const rows = await liveQueryClient.execute<ReconcileRow>(`
    SELECT
      attachment.id,
      attachment.session_id,
      attachment.workspace_id,
      attachment.sha256,
      attachment.size_bytes,
      attachment.cloud_object_key,
      attachment.cloud_sync_enabled,
      attachment.deleted_at,
      COALESCE(local.availability, 'absent') AS local_availability
    FROM session_attachments AS attachment
    LEFT JOIN attachment_local_state AS local
      ON local.attachment_id = attachment.id
    WHERE length(attachment.sha256) = 64
      AND attachment.sha256 NOT GLOB '*[^0-9a-f]*'
      AND (
        (
          attachment.cloud_object_key <> ''
          AND (
            attachment.deleted_at IS NOT NULL
            OR attachment.cloud_sync_enabled = 0
          )
        )
        OR (
          attachment.deleted_at IS NULL
          AND attachment.cloud_sync_enabled = 1
          AND (
            (
              attachment.cloud_object_key = ''
              AND COALESCE(local.availability, 'absent') = 'present'
            )
            OR (
              attachment.cloud_object_key <> ''
              AND COALESCE(local.availability, 'absent') <> 'present'
            )
          )
        )
      )
    ORDER BY attachment.updated_at, attachment.id
  `);

  const obsoleteDownloads =
    await liveQueryClient.execute<ObsoleteDownloadJobRow>(`
      SELECT job.id, job.attempt_count
      FROM attachment_transfer_jobs AS job
      WHERE job.direction = 'download'
        AND job.phase IN ('queued', 'retry_wait', 'failed')
        AND NOT EXISTS (
          SELECT 1
          FROM session_attachments AS attachment
          LEFT JOIN attachment_local_state AS local
            ON local.attachment_id = attachment.id
          WHERE attachment.id = job.attachment_id
            AND attachment.session_id = job.session_id
            AND attachment.workspace_id = job.workspace_id
            AND attachment.sha256 = job.expected_sha256
            AND attachment.size_bytes = job.expected_size_bytes
            AND attachment.cloud_object_key = job.object_key
            AND attachment.deleted_at IS NULL
            AND COALESCE(local.availability, 'absent') <> 'present'
        )
      ORDER BY job.created_at, job.id
  `);

  const now = new Date().toISOString();
  const statements: Array<{ sql: string; params: unknown[] }> =
    obsoleteDownloads.map((job) => ({
      sql: `
      UPDATE attachment_transfer_jobs AS job
      SET
        phase = 'completed',
        completed_at = ?,
        last_error = '',
        updated_at = ?
      WHERE job.id = ?
        AND job.attempt_count = ?
        AND job.direction = 'download'
        AND job.phase IN ('queued', 'retry_wait', 'failed')
        AND NOT EXISTS (
          SELECT 1
          FROM session_attachments AS attachment
          LEFT JOIN attachment_local_state AS local
            ON local.attachment_id = attachment.id
          WHERE attachment.id = job.attachment_id
            AND attachment.session_id = job.session_id
            AND attachment.workspace_id = job.workspace_id
            AND attachment.sha256 = job.expected_sha256
            AND attachment.size_bytes = job.expected_size_bytes
            AND attachment.cloud_object_key = job.object_key
            AND attachment.deleted_at IS NULL
            AND COALESCE(local.availability, 'absent') <> 'present'
        )
    `,
      params: [now, now, job.id, job.attempt_count],
    }));
  statements.push(
    ...rows.map((row) => {
      const direction: AttachmentTransferDirection =
        row.deleted_at !== null
          ? "delete"
          : row.cloud_object_key && row.local_availability !== "present"
            ? "download"
            : !Boolean(row.cloud_sync_enabled)
              ? "delete"
              : row.cloud_object_key
                ? "download"
                : "upload";
      return {
        sql: `
        INSERT OR IGNORE INTO attachment_transfer_jobs (
          id,
          attachment_id,
          session_id,
          workspace_id,
          direction,
          expected_sha256,
          expected_size_bytes,
          object_key
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      `,
        params: [
          id(),
          row.id,
          row.session_id,
          row.workspace_id,
          direction,
          row.sha256,
          row.size_bytes,
          direction === "upload" ? "" : row.cloud_object_key,
        ],
      };
    }),
  );
  if (statements.length === 0) return 0;

  const results = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction(statements),
  );
  return results.reduce((total, count) => total + count, 0);
}

export async function recoverInterruptedAttachmentTransfers(
  processLocalActiveAttempts: ReadonlyArray<{
    id: string;
    attemptCount: number;
  }> = [],
  staleBefore = new Date(Date.now() - 15 * 60 * 1000).toISOString(),
): Promise<number> {
  const now = new Date().toISOString();
  const activeAttemptFence = processLocalActiveAttempts.length
    ? `AND NOT (${processLocalActiveAttempts
        .map(() => "(id = ? AND attempt_count = ?)")
        .join(" OR ")})`
    : "";
  const [count = 0] = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
            UPDATE attachment_transfer_jobs
            SET
              phase = 'retry_wait',
              attempt_count = attempt_count + 1,
              next_attempt_at = ?,
              last_error = 'The previous transfer was interrupted.',
              updated_at = ?
            WHERE phase IN (${ACTIVE_PHASES})
              AND updated_at < ?
              ${activeAttemptFence}
          `,
        params: [
          now,
          now,
          staleBefore,
          ...processLocalActiveAttempts.flatMap(({ id, attemptCount }) => [
            id,
            attemptCount,
          ]),
        ],
      },
    ]),
  );
  return count;
}

export async function resetProcessLocalAttachmentTransferAttempts(): Promise<number> {
  const now = new Date().toISOString();
  const [count = 0] = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
          UPDATE attachment_transfer_jobs
          SET
            phase = 'retry_wait',
            attempt_count = attempt_count + 1,
            cache_id = CASE WHEN direction = 'delete' THEN cache_id ELSE '' END,
            next_attempt_at = ?,
            last_error = 'The previous process-local transfer was interrupted.',
            updated_at = ?
          WHERE phase IN (${ACTIVE_PHASES})
        `,
        params: [now, now],
      },
    ]),
  );
  return count;
}
