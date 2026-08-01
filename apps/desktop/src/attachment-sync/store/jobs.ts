import {
  type AttachmentTransferDirection,
  type AttachmentTransferJob,
  type AttachmentTransferPhase,
  type JobRow,
} from "./types";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

export async function claimNextAttachmentTransferJob(): Promise<
  AttachmentTransferJob | undefined
> {
  const now = new Date().toISOString();
  const [candidate] = await liveQueryClient.execute<{
    id: string;
    attempt_count: number;
  }>(
    `
      SELECT id, attempt_count
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
              AND attempt_count = ?
              AND phase IN ('queued', 'retry_wait')
              AND next_attempt_at <= ?
          `,
        params: [now, now, candidate.id, candidate.attempt_count, now],
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
        AND job.attempt_count = ?
        AND job.phase = 'preparing'
      LIMIT 1
    `,
    [candidate.id, candidate.attempt_count + 1],
  );
  return row ? parseJob(row) : undefined;
}

export function setUploadReservation(
  job: AttachmentTransferJob,
  input: { objectId: string; objectKey: string },
) {
  return updateJob(job, "preparing", {
    remote_object_id: input.objectId,
    object_key: input.objectKey,
  });
}

export function setDownloadGrant(
  job: AttachmentTransferJob,
  input: {
    objectId: string;
    ciphertextSha256: string;
    ciphertextSizeBytes: number;
  },
) {
  return updateJob(job, "transferring", {
    remote_object_id: input.objectId,
    ciphertext_sha256: input.ciphertextSha256,
    ciphertext_size_bytes: input.ciphertextSizeBytes,
  });
}

export function markPhase(
  job: AttachmentTransferJob,
  phase: AttachmentTransferPhase,
) {
  return updateJob(job, phase, {});
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
          WHERE id = ? AND attempt_count = ? AND phase <> 'completed'
        `,
        params: [
          retryAt.toISOString(),
          boundedError(message),
          now,
          job.id,
          job.attemptCount,
        ],
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
          WHERE id = ? AND attempt_count = ? AND phase <> 'completed'
        `,
        params: [boundedError(message), now, job.id, job.attemptCount],
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
            attempt_count = attempt_count + 1,
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
  job: AttachmentTransferJob,
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
            WHERE id = ? AND attempt_count = ? AND phase <> 'completed'
          `,
        params: [
          ...entries.map(([, value]) => value),
          phase,
          now,
          job.id,
          job.attemptCount,
        ],
      },
    ]),
  );
  if (count !== 1) throw new Error("Attachment transfer is no longer active");
}

export function completedJobStatement(job: AttachmentTransferJob, now: string) {
  return {
    sql: `
      UPDATE attachment_transfer_jobs
      SET phase = 'completed', completed_at = ?, updated_at = ?, last_error = ''
      WHERE id = ? AND attempt_count = ? AND phase <> 'completed'
    `,
    params: [now, now, job.id, job.attemptCount],
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
