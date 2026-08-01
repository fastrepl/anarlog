import { completedJobStatement } from "./jobs";
import { ACTIVE_PHASES, type AttachmentTransferJob } from "./types";

import { executeTransaction } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

function deleteIntent(job: AttachmentTransferJob) {
  const attachmentIdentityParams = [
    job.attachmentId,
    job.sessionId,
    job.workspaceId,
  ];
  return {
    sql: `(
      NOT EXISTS (
        SELECT 1
        FROM session_attachments AS attachment
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.workspace_id = ?
      )
      OR EXISTS (
        SELECT 1
        FROM session_attachments AS attachment
        LEFT JOIN attachment_local_state AS local
          ON local.attachment_id = attachment.id
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.workspace_id = ?
          AND (
            attachment.deleted_at IS NOT NULL
            OR (
              attachment.cloud_object_key = ?
              AND attachment.cloud_sync_enabled = 0
              AND COALESCE(local.availability, 'absent') = 'present'
            )
            OR (
              attachment.cloud_object_key <> ?
              AND (
                attachment.cloud_sync_enabled = 0
                OR attachment.cloud_object_key <> ''
                OR attachment.sha256 <> ?
                OR attachment.size_bytes <> ?
              )
            )
          )
      )
    )`,
    params: [
      ...attachmentIdentityParams,
      ...attachmentIdentityParams,
      job.objectKey,
      job.objectKey,
      job.expectedSha256,
      job.expectedSizeBytes,
    ],
  };
}

export async function prepareAttachmentTransferDelete(
  job: AttachmentTransferJob,
): Promise<boolean> {
  const now = new Date().toISOString();
  const intent = deleteIntent(job);
  const [prepared = 0, superseded = 0] = await enqueueDatabaseWrite(
    "attachment-transfers",
    () =>
      executeTransaction([
        {
          sql: `
            UPDATE attachment_transfer_jobs
            SET phase = 'finalizing', updated_at = ?
            WHERE id = ?
              AND attempt_count = ?
              AND direction = 'delete'
              AND phase IN (${ACTIVE_PHASES})
              AND ${intent.sql}
          `,
          params: [now, job.id, job.attemptCount, ...intent.params],
        },
        {
          sql: `
            UPDATE attachment_transfer_jobs
            SET phase = 'finalizing', updated_at = ?
            WHERE id = ?
              AND attempt_count = ?
              AND direction = 'delete'
              AND phase IN (${ACTIVE_PHASES})
              AND NOT ${intent.sql}
          `,
          params: [now, job.id, job.attemptCount, ...intent.params],
        },
      ]),
  );
  if (prepared === 1 && superseded === 0) return true;
  if (prepared === 0 && superseded === 1) return false;
  throw new Error("Attachment transfer is no longer active");
}

export async function completeCancelledAttachmentTransferDelete(
  job: AttachmentTransferJob,
): Promise<void> {
  const now = new Date().toISOString();
  const intent = deleteIntent(job);
  const replacementId = id();
  const [completed = 0, restarted = 0, replacement = 0] =
    await enqueueDatabaseWrite("attachment-transfers", () =>
      executeTransaction([
        {
          sql: `
            UPDATE attachment_transfer_jobs
            SET
              phase = 'completed',
              cache_id = '',
              ciphertext_sha256 = '',
              ciphertext_size_bytes = 0,
              completed_at = ?,
              updated_at = ?,
              last_error = ''
            WHERE id = ?
              AND attempt_count = ?
              AND direction = 'delete'
              AND phase = 'finalizing'
              AND attachment_id = ?
              AND session_id = ?
              AND workspace_id = ?
              AND expected_sha256 = ?
              AND expected_size_bytes = ?
              AND object_key = ?
              AND NOT ${intent.sql}
          `,
          params: [
            now,
            now,
            job.id,
            job.attemptCount,
            job.attachmentId,
            job.sessionId,
            job.workspaceId,
            job.expectedSha256,
            job.expectedSizeBytes,
            job.objectKey,
            ...intent.params,
          ],
        },
        {
          sql: `
            UPDATE attachment_transfer_jobs
            SET
              phase = 'completed',
              cache_id = '',
              ciphertext_sha256 = '',
              ciphertext_size_bytes = 0,
              completed_at = ?,
              updated_at = ?,
              last_error = ''
            WHERE id = ?
              AND attempt_count = ?
              AND direction = 'delete'
              AND phase = 'finalizing'
              AND attachment_id = ?
              AND session_id = ?
              AND workspace_id = ?
              AND expected_sha256 = ?
              AND expected_size_bytes = ?
              AND object_key = ?
              AND ${intent.sql}
          `,
          params: [
            now,
            now,
            job.id,
            job.attemptCount,
            job.attachmentId,
            job.sessionId,
            job.workspaceId,
            job.expectedSha256,
            job.expectedSizeBytes,
            job.objectKey,
            ...intent.params,
          ],
        },
        {
          sql: `
            INSERT INTO attachment_transfer_jobs (
              id,
              attachment_id,
              session_id,
              workspace_id,
              direction,
              expected_sha256,
              expected_size_bytes,
              object_key
            )
            SELECT ?, job.attachment_id, job.session_id, job.workspace_id,
              'delete', job.expected_sha256, job.expected_size_bytes,
              job.object_key
            FROM attachment_transfer_jobs AS job
            WHERE job.id = ?
              AND job.attempt_count = ?
              AND job.direction = 'delete'
              AND job.phase = 'completed'
              AND job.completed_at = ?
              AND job.attachment_id = ?
              AND job.session_id = ?
              AND job.workspace_id = ?
              AND job.expected_sha256 = ?
              AND job.expected_size_bytes = ?
              AND job.object_key = ?
              AND ${intent.sql}
          `,
          params: [
            replacementId,
            job.id,
            job.attemptCount,
            now,
            job.attachmentId,
            job.sessionId,
            job.workspaceId,
            job.expectedSha256,
            job.expectedSizeBytes,
            job.objectKey,
            ...intent.params,
          ],
        },
      ]),
    );
  if (
    !(
      (completed === 1 && restarted === 0 && replacement === 0) ||
      (completed === 0 && restarted === 1 && replacement === 1)
    )
  ) {
    throw new Error("Attachment transfer is no longer cancelled");
  }
}

export async function completeUpload(
  job: AttachmentTransferJob,
  objectKey: string,
): Promise<boolean> {
  const now = new Date().toISOString();
  const [updated = 0, , completed = 0] = await enqueueDatabaseWrite(
    "attachment-transfers",
    () =>
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
              AND EXISTS (
                SELECT 1
                FROM attachment_transfer_jobs AS job
                WHERE job.id = ?
                  AND job.attempt_count = ?
                  AND job.direction = 'upload'
                  AND job.phase IN (${ACTIVE_PHASES})
              )
          `,
          params: [
            objectKey,
            now,
            job.attachmentId,
            job.sessionId,
            job.expectedSha256,
            job.expectedSizeBytes,
            job.id,
            job.attemptCount,
          ],
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
              expected_size_bytes,
              object_key
            )
            SELECT ?, job.attachment_id, job.session_id, job.workspace_id,
              'delete', job.expected_sha256, job.expected_size_bytes, ?
            FROM attachment_transfer_jobs AS job
            WHERE job.id = ?
              AND job.attempt_count = ?
              AND job.direction = 'upload'
              AND job.phase IN (${ACTIVE_PHASES})
              AND NOT EXISTS (
                SELECT 1
                FROM session_attachments AS attachment
                WHERE attachment.id = job.attachment_id
                  AND attachment.session_id = job.session_id
                  AND attachment.workspace_id = job.workspace_id
                  AND attachment.sha256 = job.expected_sha256
                  AND attachment.size_bytes = job.expected_size_bytes
                  AND attachment.cloud_object_key = ?
              )
          `,
          params: [id(), objectKey, job.id, job.attemptCount, objectKey],
        },
        completedJobStatement(job, now),
      ]),
  );
  if (completed !== 1) {
    throw new Error("Attachment transfer is no longer active");
  }
  return updated === 1;
}

export async function deferAttachmentTransferDeleteForPreservation(
  job: AttachmentTransferJob,
): Promise<void> {
  const now = new Date().toISOString();
  const exactDeleteParams = [
    job.id,
    job.attemptCount,
    job.attachmentId,
    job.sessionId,
    job.workspaceId,
    job.expectedSha256,
    job.expectedSizeBytes,
    job.objectKey,
  ];
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([
      {
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
            )
            SELECT ?, job.attachment_id, job.session_id, job.workspace_id,
              'download', job.expected_sha256, job.expected_size_bytes,
              job.object_key
            FROM attachment_transfer_jobs AS job
            JOIN session_attachments AS attachment
              ON attachment.id = job.attachment_id
              AND attachment.session_id = job.session_id
              AND attachment.workspace_id = job.workspace_id
              AND attachment.sha256 = job.expected_sha256
              AND attachment.size_bytes = job.expected_size_bytes
              AND attachment.cloud_object_key = job.object_key
              AND attachment.deleted_at IS NULL
            WHERE job.id = ?
              AND job.attempt_count = ?
              AND job.direction = 'delete'
              AND job.phase = 'finalizing'
              AND job.attachment_id = ?
              AND job.session_id = ?
              AND job.workspace_id = ?
              AND job.expected_sha256 = ?
              AND job.expected_size_bytes = ?
              AND job.object_key = ?
          `,
        params: [id(), ...exactDeleteParams],
      },
      {
        sql: `
            INSERT INTO attachment_local_state (
              attachment_id,
              session_id,
              relative_path,
              availability,
              updated_at
            )
            SELECT attachment.id, attachment.session_id,
              attachment.relative_path, 'absent', ?
            FROM attachment_transfer_jobs AS job
            JOIN session_attachments AS attachment
              ON attachment.id = job.attachment_id
              AND attachment.session_id = job.session_id
              AND attachment.workspace_id = job.workspace_id
              AND attachment.sha256 = job.expected_sha256
              AND attachment.size_bytes = job.expected_size_bytes
              AND attachment.cloud_object_key = job.object_key
              AND attachment.deleted_at IS NULL
            WHERE job.id = ?
              AND job.attempt_count = ?
              AND job.direction = 'delete'
              AND job.phase = 'finalizing'
              AND job.attachment_id = ?
              AND job.session_id = ?
              AND job.workspace_id = ?
              AND job.expected_sha256 = ?
              AND job.expected_size_bytes = ?
              AND job.object_key = ?
              AND EXISTS (
                SELECT 1
                FROM attachment_transfer_jobs AS preservation
                WHERE preservation.attachment_id = job.attachment_id
                  AND preservation.session_id = job.session_id
                  AND preservation.workspace_id = job.workspace_id
                  AND preservation.direction = 'download'
                  AND preservation.expected_sha256 = job.expected_sha256
                  AND preservation.expected_size_bytes = job.expected_size_bytes
                  AND preservation.object_key = job.object_key
                  AND preservation.phase <> 'completed'
              )
            ON CONFLICT(attachment_id) DO UPDATE SET
              session_id = excluded.session_id,
              relative_path = excluded.relative_path,
              availability = excluded.availability,
              updated_at = excluded.updated_at
          `,
        params: [now, ...exactDeleteParams],
        expectedRowsAffected: 1,
      },
      {
        sql: `
            UPDATE attachment_transfer_jobs AS job
            SET
              phase = 'completed',
              completed_at = ?,
              updated_at = ?,
              last_error = ''
            WHERE job.id = ?
              AND job.attempt_count = ?
              AND job.direction = 'delete'
              AND job.phase = 'finalizing'
              AND job.attachment_id = ?
              AND job.session_id = ?
              AND job.workspace_id = ?
              AND job.expected_sha256 = ?
              AND job.expected_size_bytes = ?
              AND job.object_key = ?
              AND EXISTS (
                SELECT 1
                FROM session_attachments AS attachment
                JOIN attachment_local_state AS local
                  ON local.attachment_id = attachment.id
                  AND local.session_id = attachment.session_id
                  AND local.relative_path = attachment.relative_path
                  AND local.availability = 'absent'
                WHERE attachment.id = job.attachment_id
                  AND attachment.session_id = job.session_id
                  AND attachment.workspace_id = job.workspace_id
                  AND attachment.sha256 = job.expected_sha256
                  AND attachment.size_bytes = job.expected_size_bytes
                  AND attachment.cloud_object_key = job.object_key
                  AND attachment.deleted_at IS NULL
              )
              AND EXISTS (
                SELECT 1
                FROM attachment_transfer_jobs AS preservation
                WHERE preservation.attachment_id = job.attachment_id
                  AND preservation.session_id = job.session_id
                  AND preservation.workspace_id = job.workspace_id
                  AND preservation.direction = 'download'
                  AND preservation.expected_sha256 = job.expected_sha256
                  AND preservation.expected_size_bytes = job.expected_size_bytes
                  AND preservation.object_key = job.object_key
                  AND preservation.phase <> 'completed'
              )
          `,
        params: [now, now, ...exactDeleteParams],
        expectedRowsAffected: 1,
      },
    ]),
  );
}

export async function completeWithoutTransfer(
  job: AttachmentTransferJob,
): Promise<void> {
  const now = new Date().toISOString();
  await enqueueDatabaseWrite("attachment-transfers", () =>
    executeTransaction([completedJobStatement(job, now)]),
  );
}
