import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

export type AttachmentTransferDirection = "upload" | "download" | "delete";
export type AttachmentTransferPhase =
  | "queued"
  | "preparing"
  | "ready"
  | "transferring"
  | "finalizing"
  | "retry_wait"
  | "failed"
  | "completed";

export type AttachmentTransferJob = {
  id: string;
  attachmentId: string;
  sessionId: string;
  workspaceId: string;
  direction: AttachmentTransferDirection;
  expectedSha256: string;
  expectedSizeBytes: number;
  ciphertextSha256: string;
  ciphertextSizeBytes: number;
  remoteObjectId: string;
  objectKey: string;
  cacheId: string;
  phase: AttachmentTransferPhase;
  attemptCount: number;
  cloudSyncEnabled: boolean;
  currentObjectKey: string;
  attachmentDeleted: boolean;
  localAvailability: "present" | "absent";
  attachmentVersionMatches: boolean;
};

type ReconcileRow = {
  id: string;
  session_id: string;
  workspace_id: string;
  sha256: string;
  size_bytes: number;
  cloud_object_key: string;
  cloud_sync_enabled: number | boolean;
  deleted_at: string | null;
  local_availability: string;
};

type JobRow = {
  id: string;
  attachment_id: string;
  session_id: string;
  workspace_id: string;
  direction: string;
  expected_sha256: string;
  expected_size_bytes: number;
  ciphertext_sha256: string;
  ciphertext_size_bytes: number;
  remote_object_id: string;
  object_key: string;
  cache_id: string;
  phase: string;
  attempt_count: number;
  cloud_sync_enabled: number | boolean | null;
  current_object_key: string | null;
  attachment_deleted: number | boolean | null;
  local_availability: string | null;
  attachment_version_matches: number | boolean | null;
};

type ObsoleteDownloadJobRow = {
  id: string;
};

const ACTIVE_PHASES = "'preparing', 'ready', 'transferring', 'finalizing'";

export const attachmentTransferStore = {
  reconcile: reconcileAttachmentTransferJobs,
  recoverInterrupted: recoverInterruptedAttachmentTransfers,
  claimNext: claimNextAttachmentTransferJob,
  setUploadReservation,
  setDownloadGrant,
  markPhase,
  completeUpload,
  completeDelete,
  completeWithoutTransfer,
  retry: retryAttachmentTransferJob,
  fail: failAttachmentTransferJob,
};

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
      SELECT job.id
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
            AND attachment.cloud_sync_enabled = 1
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
            AND attachment.cloud_sync_enabled = 1
            AND attachment.deleted_at IS NULL
            AND COALESCE(local.availability, 'absent') <> 'present'
        )
    `,
      params: [now, now, job.id],
    }));
  statements.push(
    ...rows.map((row) => {
      const direction: AttachmentTransferDirection =
        row.deleted_at !== null || !Boolean(row.cloud_sync_enabled)
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
  staleBefore = new Date(Date.now() - 15 * 60 * 1000).toISOString(),
): Promise<number> {
  const now = new Date().toISOString();
  const [count = 0] = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
            UPDATE attachment_transfer_jobs
            SET
              phase = 'retry_wait',
              next_attempt_at = ?,
              last_error = 'The previous transfer was interrupted.',
              updated_at = ?
            WHERE phase IN (${ACTIVE_PHASES})
              AND updated_at < ?
          `,
        params: [now, now, staleBefore],
      },
    ]),
  );
  return count;
}

export async function claimNextAttachmentTransferJob(): Promise<
  AttachmentTransferJob | undefined
> {
  const now = new Date().toISOString();
  const [candidate] = await liveQueryClient.execute<{ id: string }>(
    `
      SELECT id
      FROM attachment_transfer_jobs
      WHERE phase IN ('queued', 'retry_wait')
        AND next_attempt_at <= ?
      ORDER BY next_attempt_at, created_at, id
      LIMIT 1
    `,
    [now],
  );
  if (!candidate) return undefined;

  const [claimed = 0] = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
            UPDATE attachment_transfer_jobs
            SET
              phase = 'preparing',
              attempt_count = attempt_count + 1,
              last_attempt_at = ?,
              last_error = '',
              updated_at = ?
            WHERE id = ?
              AND phase IN ('queued', 'retry_wait')
              AND next_attempt_at <= ?
          `,
        params: [now, now, candidate.id, now],
      },
    ]),
  );
  if (claimed !== 1) return undefined;

  const [row] = await liveQueryClient.execute<JobRow>(
    `
      SELECT
        job.*,
        attachment.cloud_sync_enabled,
        attachment.cloud_object_key AS current_object_key,
        attachment.deleted_at IS NOT NULL AS attachment_deleted,
        (
          attachment.sha256 = job.expected_sha256
          AND attachment.size_bytes = job.expected_size_bytes
        ) AS attachment_version_matches,
        COALESCE(local.availability, 'absent') AS local_availability
      FROM attachment_transfer_jobs AS job
      LEFT JOIN session_attachments AS attachment
        ON attachment.id = job.attachment_id
      LEFT JOIN attachment_local_state AS local
        ON local.attachment_id = job.attachment_id
      WHERE job.id = ?
      LIMIT 1
    `,
    [candidate.id],
  );
  return row ? parseJob(row) : undefined;
}

export function setUploadReservation(
  jobId: string,
  input: { objectId: string; objectKey: string },
) {
  return updateJob(jobId, "preparing", {
    remote_object_id: input.objectId,
    object_key: input.objectKey,
  });
}

export function setDownloadGrant(
  jobId: string,
  input: {
    objectId: string;
    ciphertextSha256: string;
    ciphertextSizeBytes: number;
  },
) {
  return updateJob(jobId, "transferring", {
    remote_object_id: input.objectId,
    ciphertext_sha256: input.ciphertextSha256,
    ciphertext_size_bytes: input.ciphertextSizeBytes,
  });
}

export function markPhase(jobId: string, phase: AttachmentTransferPhase) {
  return updateJob(jobId, phase, {});
}

export async function completeUpload(
  job: AttachmentTransferJob,
  objectKey: string,
): Promise<boolean> {
  const now = new Date().toISOString();
  const [updated = 0] = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
            UPDATE session_attachments
            SET
              storage_kind = 'private_cloud',
              cloud_object_key = ?,
              updated_at = ?
            WHERE id = ?
              AND session_id = ?
              AND sha256 = ?
              AND size_bytes = ?
              AND cloud_sync_enabled = 1
              AND deleted_at IS NULL
          `,
        params: [
          objectKey,
          now,
          job.attachmentId,
          job.sessionId,
          job.expectedSha256,
          job.expectedSizeBytes,
        ],
      },
      completedJobStatement(job.id, now),
    ]),
  );
  return updated === 1;
}

export async function completeDelete(
  job: AttachmentTransferJob,
): Promise<void> {
  const now = new Date().toISOString();
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
          UPDATE session_attachments
          SET
            storage_kind = 'local_file',
            cloud_object_key = '',
            updated_at = ?
          WHERE id = ?
            AND cloud_object_key = ?
        `,
        params: [now, job.attachmentId, job.objectKey],
      },
      {
        sql: `
          INSERT OR IGNORE INTO attachment_transfer_jobs (
            id,
            attachment_id,
            session_id,
            workspace_id,
            direction,
            expected_sha256,
            expected_size_bytes
          )
          SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
            'upload', attachment.sha256, attachment.size_bytes
          FROM session_attachments AS attachment
          JOIN attachment_local_state AS local
            ON local.attachment_id = attachment.id
            AND local.availability = 'present'
          WHERE attachment.id = ?
            AND attachment.cloud_sync_enabled = 1
            AND attachment.cloud_object_key = ''
            AND attachment.deleted_at IS NULL
        `,
        params: [id(), job.attachmentId],
      },
      completedJobStatement(job.id, now),
    ]),
  );
}

export async function completeWithoutTransfer(jobId: string): Promise<void> {
  const now = new Date().toISOString();
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([completedJobStatement(jobId, now)]),
  );
}

export async function retryAttachmentTransferJob(
  job: AttachmentTransferJob,
  message: string,
  retryAt: Date,
): Promise<void> {
  const now = new Date().toISOString();
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
          UPDATE attachment_transfer_jobs
          SET
            phase = 'retry_wait',
            next_attempt_at = ?,
            last_error = ?,
            updated_at = ?
          WHERE id = ? AND phase <> 'completed'
        `,
        params: [retryAt.toISOString(), boundedError(message), now, job.id],
      },
    ]),
  );
}

export async function failAttachmentTransferJob(
  job: AttachmentTransferJob,
  message: string,
): Promise<void> {
  const now = new Date().toISOString();
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
          UPDATE attachment_transfer_jobs
          SET phase = 'failed', last_error = ?, updated_at = ?
          WHERE id = ? AND phase <> 'completed'
        `,
        params: [boundedError(message), now, job.id],
      },
    ]),
  );
}

export async function retryAttachmentTransfersForAttachment(
  attachmentId: string,
): Promise<void> {
  const now = new Date().toISOString();
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
          UPDATE attachment_transfer_jobs
          SET
            phase = 'queued',
            attempt_count = 0,
            next_attempt_at = ?,
            last_error = '',
            updated_at = ?
          WHERE attachment_id = ? AND phase = 'failed'
        `,
        params: [now, now, attachmentId],
      },
    ]),
  );
}

async function updateJob(
  jobId: string,
  phase: AttachmentTransferPhase,
  values: Record<string, string | number>,
) {
  const now = new Date().toISOString();
  const entries = Object.entries(values);
  const assignments = entries.map(([column]) => `${column} = ?`);
  const [count = 0] = await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
        sql: `
            UPDATE attachment_transfer_jobs
            SET ${[...assignments, "phase = ?", "updated_at = ?"].join(", ")}
            WHERE id = ? AND phase <> 'completed'
          `,
        params: [...entries.map(([, value]) => value), phase, now, jobId],
      },
    ]),
  );
  if (count !== 1) throw new Error("Attachment transfer is no longer active");
}

function completedJobStatement(jobId: string, now: string) {
  return {
    sql: `
      UPDATE attachment_transfer_jobs
      SET phase = 'completed', completed_at = ?, updated_at = ?, last_error = ''
      WHERE id = ?
    `,
    params: [now, now, jobId],
  };
}

function parseJob(row: JobRow): AttachmentTransferJob {
  return {
    id: row.id,
    attachmentId: row.attachment_id,
    sessionId: row.session_id,
    workspaceId: row.workspace_id,
    direction: row.direction as AttachmentTransferDirection,
    expectedSha256: row.expected_sha256,
    expectedSizeBytes: Number(row.expected_size_bytes),
    ciphertextSha256: row.ciphertext_sha256,
    ciphertextSizeBytes: Number(row.ciphertext_size_bytes),
    remoteObjectId: row.remote_object_id,
    objectKey: row.object_key,
    cacheId: row.cache_id,
    phase: row.phase as AttachmentTransferPhase,
    attemptCount: Number(row.attempt_count),
    cloudSyncEnabled: Boolean(row.cloud_sync_enabled),
    currentObjectKey: row.current_object_key ?? "",
    attachmentDeleted: Boolean(row.attachment_deleted),
    localAvailability:
      row.local_availability === "present" ? "present" : "absent",
    attachmentVersionMatches: Boolean(row.attachment_version_matches),
  };
}

function boundedError(value: string) {
  return value.slice(0, 2048);
}
