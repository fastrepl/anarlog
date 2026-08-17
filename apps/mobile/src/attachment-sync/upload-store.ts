import { execute, executeTransaction } from "@/db";
import { id, nowIso } from "@/lib/ids";

export type MobileAttachmentUploadJob = {
  id: string;
  attachmentId: string;
  sessionId: string;
  workspaceId: string;
  expectedSha256: string;
  expectedSizeBytes: number;
  cacheId: string;
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
};

type JobRow = {
  id: string;
  attachment_id: string;
  session_id: string;
  workspace_id: string;
  expected_sha256: string;
  expected_size_bytes: number;
  cache_id: string;
  attempt_count: number;
  cloud_sync_enabled: number | boolean | null;
  current_object_key: string | null;
  attachment_deleted: number | boolean | null;
  local_availability: string | null;
  attachment_version_matches: number | boolean | null;
};

const ACTIVE_PHASES = "'preparing', 'ready', 'transferring', 'finalizing'";

export async function reconcileMobileAttachmentUploads(): Promise<number> {
  const rows = await execute<ReconcileRow>(`
    SELECT
      attachment.id,
      attachment.session_id,
      attachment.workspace_id,
      attachment.sha256,
      attachment.size_bytes
    FROM session_attachments AS attachment
    JOIN attachment_local_state AS local
      ON local.attachment_id = attachment.id
      AND local.availability = 'present'
    WHERE attachment.source_type IN ('session_audio', 'note_upload')
      AND attachment.deleted_at IS NULL
      AND attachment.cloud_sync_enabled = 1
      AND attachment.cloud_object_key = ''
      AND attachment.size_bytes > 0
      AND attachment.size_bytes <= 536870912
      AND length(attachment.sha256) = 64
      AND attachment.sha256 NOT GLOB '*[^0-9a-f]*'
      AND NOT EXISTS (
        SELECT 1 FROM attachment_transfer_jobs AS job
        WHERE job.attachment_id = attachment.id
          AND job.expected_sha256 = attachment.sha256
          AND job.expected_size_bytes = attachment.size_bytes
          AND job.direction = 'upload'
          AND job.phase <> 'completed'
      )
    ORDER BY attachment.updated_at, attachment.id
  `);
  if (rows.length === 0) return 0;

  const results = await executeTransaction(
    rows.map((row) => ({
      sql: `
        INSERT OR IGNORE INTO attachment_transfer_jobs (
          id, attachment_id, session_id, workspace_id, direction,
          expected_sha256, expected_size_bytes
        ) VALUES (?, ?, ?, ?, 'upload', ?, ?)
      `,
      params: [
        id(),
        row.id,
        row.session_id,
        row.workspace_id,
        row.sha256,
        row.size_bytes,
      ],
    })),
  );
  return results.reduce((total, count) => total + count, 0);
}

export async function resetInterruptedMobileAttachmentUploads(): Promise<number> {
  const now = nowIso();
  const [count = 0] = await executeTransaction([
    {
      sql: `
        UPDATE attachment_transfer_jobs
        SET
          phase = 'retry_wait',
          attempt_count = attempt_count + 1,
          next_attempt_at = ?,
          last_error = 'The previous mobile upload was interrupted.',
          updated_at = ?
        WHERE direction = 'upload' AND phase IN (${ACTIVE_PHASES})
      `,
      params: [now, now],
    },
  ]);
  return count;
}

export async function claimNextMobileAttachmentUpload(): Promise<
  MobileAttachmentUploadJob | undefined
> {
  const now = nowIso();
  const [candidate] = await execute<{ id: string; attempt_count: number }>(
    `
      SELECT id, attempt_count
      FROM attachment_transfer_jobs
      WHERE direction = 'upload'
        AND phase IN ('queued', 'retry_wait')
        AND next_attempt_at <= ?
      ORDER BY next_attempt_at, created_at, id
      LIMIT 1
    `,
    [now],
  );
  if (!candidate) return undefined;

  const [claimed = 0] = await executeTransaction([
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
          AND direction = 'upload'
          AND attempt_count = ?
          AND phase IN ('queued', 'retry_wait')
          AND next_attempt_at <= ?
      `,
      params: [now, now, candidate.id, candidate.attempt_count, now],
    },
  ]);
  if (claimed !== 1) return undefined;

  const [row] = await execute<JobRow>(
    `
      SELECT
        job.id,
        job.attachment_id,
        job.session_id,
        job.workspace_id,
        job.expected_sha256,
        job.expected_size_bytes,
        job.cache_id,
        job.attempt_count,
        attachment.cloud_sync_enabled,
        attachment.cloud_object_key AS current_object_key,
        attachment.deleted_at IS NOT NULL AS attachment_deleted,
        COALESCE(local.availability, 'absent') AS local_availability,
        (
          attachment.sha256 = job.expected_sha256
          AND attachment.size_bytes = job.expected_size_bytes
        ) AS attachment_version_matches
      FROM attachment_transfer_jobs AS job
      LEFT JOIN session_attachments AS attachment
        ON attachment.id = job.attachment_id
      LEFT JOIN attachment_local_state AS local
        ON local.attachment_id = job.attachment_id
      WHERE job.id = ?
        AND job.attempt_count = ?
        AND job.direction = 'upload'
        AND job.phase = 'preparing'
      LIMIT 1
    `,
    [candidate.id, candidate.attempt_count + 1],
  );
  return row ? parseJob(row) : undefined;
}

export async function setMobileAttachmentUploadReservation(
  job: MobileAttachmentUploadJob,
  input: { objectId: string; objectKey: string },
): Promise<void> {
  await updateJob(
    job,
    "preparing",
    ["remote_object_id = ?", "object_key = ?"],
    [input.objectId, input.objectKey],
  );
}

export async function markMobileAttachmentUploadPhase(
  job: MobileAttachmentUploadJob,
  phase: "transferring" | "finalizing",
): Promise<void> {
  await updateJob(job, phase, [], []);
}

export async function currentMobileAttachmentUploadVersion(
  job: MobileAttachmentUploadJob,
): Promise<boolean> {
  const [row] = await execute<{ current: number | boolean }>(
    `
      SELECT EXISTS(
        SELECT 1 FROM session_attachments AS attachment
        JOIN attachment_local_state AS local
          ON local.attachment_id = attachment.id
          AND local.availability = 'present'
        WHERE attachment.id = ?
          AND attachment.session_id = ?
          AND attachment.workspace_id = ?
          AND attachment.source_type IN ('session_audio', 'note_upload')
          AND attachment.sha256 = ?
          AND attachment.size_bytes = ?
          AND attachment.cloud_sync_enabled = 1
          AND attachment.deleted_at IS NULL
      ) AS current
    `,
    [
      job.attachmentId,
      job.sessionId,
      job.workspaceId,
      job.expectedSha256,
      job.expectedSizeBytes,
    ],
  );
  return Boolean(row?.current);
}

export async function completeMobileAttachmentUpload(
  job: MobileAttachmentUploadJob,
  objectKey: string,
): Promise<boolean> {
  const now = nowIso();
  const [updated = 0, completed = 0] = await executeTransaction([
    {
      sql: `
        UPDATE session_attachments
        SET storage_kind = 'private_cloud', cloud_object_key = ?, updated_at = ?
        WHERE id = ?
          AND session_id = ?
          AND workspace_id = ?
          AND source_type IN ('session_audio', 'note_upload')
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
        job.workspaceId,
        job.expectedSha256,
        job.expectedSizeBytes,
      ],
    },
    completedStatement(job, now),
  ]);
  if (completed !== 1) {
    throw new Error("Attachment upload is no longer active");
  }
  return updated === 1;
}

export async function completeMobileAttachmentUploadWithoutTransfer(
  job: MobileAttachmentUploadJob,
): Promise<void> {
  const [completed = 0] = await executeTransaction([
    completedStatement(job, nowIso()),
  ]);
  if (completed !== 1) {
    throw new Error("Attachment upload is no longer active");
  }
}

export async function retryMobileAttachmentUpload(
  job: MobileAttachmentUploadJob,
  message: string,
  retryAt: Date,
): Promise<void> {
  const now = nowIso();
  await executeTransaction([
    {
      sql: `
        UPDATE attachment_transfer_jobs
        SET phase = 'retry_wait', next_attempt_at = ?, last_error = ?, updated_at = ?
        WHERE id = ? AND attempt_count = ? AND direction = 'upload'
          AND phase <> 'completed'
      `,
      params: [
        retryAt.toISOString(),
        message.slice(0, 2048),
        now,
        job.id,
        job.attemptCount,
      ],
    },
  ]);
}

export async function failMobileAttachmentUpload(
  job: MobileAttachmentUploadJob,
  message: string,
): Promise<void> {
  await executeTransaction([
    {
      sql: `
        UPDATE attachment_transfer_jobs
        SET phase = 'failed', last_error = ?, updated_at = ?
        WHERE id = ? AND attempt_count = ? AND direction = 'upload'
          AND phase <> 'completed'
      `,
      params: [message.slice(0, 2048), nowIso(), job.id, job.attemptCount],
    },
  ]);
}

async function updateJob(
  job: MobileAttachmentUploadJob,
  phase: "preparing" | "transferring" | "finalizing",
  assignments: string[],
  values: Array<string | number>,
) {
  const [count = 0] = await executeTransaction([
    {
      sql: `
        UPDATE attachment_transfer_jobs
        SET ${[...assignments, "phase = ?", "updated_at = ?"].join(", ")}
        WHERE id = ? AND attempt_count = ? AND direction = 'upload'
          AND phase <> 'completed'
      `,
      params: [...values, phase, nowIso(), job.id, job.attemptCount],
    },
  ]);
  if (count !== 1) throw new Error("Attachment upload is no longer active");
}

function completedStatement(job: MobileAttachmentUploadJob, now: string) {
  return {
    sql: `
      UPDATE attachment_transfer_jobs
      SET phase = 'completed', completed_at = ?, updated_at = ?, last_error = ''
      WHERE id = ? AND attempt_count = ? AND direction = 'upload'
        AND phase <> 'completed'
    `,
    params: [now, now, job.id, job.attemptCount],
  };
}

function parseJob(row: JobRow): MobileAttachmentUploadJob {
  return {
    id: row.id,
    attachmentId: row.attachment_id,
    sessionId: row.session_id,
    workspaceId: row.workspace_id,
    expectedSha256: row.expected_sha256,
    expectedSizeBytes: Number(row.expected_size_bytes),
    cacheId: row.cache_id,
    attemptCount: Number(row.attempt_count),
    cloudSyncEnabled: Boolean(row.cloud_sync_enabled),
    currentObjectKey: row.current_object_key ?? "",
    attachmentDeleted: Boolean(row.attachment_deleted),
    localAvailability:
      row.local_availability === "present" ? "present" : "absent",
    attachmentVersionMatches: Boolean(row.attachment_version_matches),
  };
}

export const mobileAttachmentUploadStore = {
  reconcile: reconcileMobileAttachmentUploads,
  resetInterrupted: resetInterruptedMobileAttachmentUploads,
  claimNext: claimNextMobileAttachmentUpload,
  setReservation: setMobileAttachmentUploadReservation,
  markPhase: markMobileAttachmentUploadPhase,
  isCurrentVersion: currentMobileAttachmentUploadVersion,
  complete: completeMobileAttachmentUpload,
  completeWithoutTransfer: completeMobileAttachmentUploadWithoutTransfer,
  retry: retryMobileAttachmentUpload,
  fail: failMobileAttachmentUpload,
};
