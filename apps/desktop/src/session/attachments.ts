import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { enqueueSessionAudioOperation } from "./audio-operations";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export async function catalogLocalNoteAttachment(input: {
  sessionId: string;
  attachmentId: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
}): Promise<void> {
  const sessionId = requireText(input.sessionId, "session ID", 512);
  const attachmentId = requireBasename(input.attachmentId, "attachment ID");
  const filename = requireBasename(input.filename, "attachment filename");
  const contentType = requireText(
    input.contentType,
    "attachment content type",
    512,
    true,
  );
  if (!Number.isSafeInteger(input.sizeBytes) || input.sizeBytes < 0) {
    throw new Error("invalid attachment size");
  }
  if (!SHA256_PATTERN.test(input.sha256)) {
    throw new Error("invalid attachment checksum");
  }

  const relativePath = `attachments/${attachmentId}`;
  const metadataId = id();
  const deleteJobId = id();
  const uploadJobId = id();
  const results = await enqueueDatabaseWrite(`session:${sessionId}`, () =>
    executeTransaction([
      enqueueReplacedAttachmentDeleteStatement({
        jobId: deleteJobId,
        sessionId,
        attachmentId: null,
        relativePath,
        nextSha256: input.sha256,
        nextSizeBytes: input.sizeBytes,
      }),
      {
        sql: `
          UPDATE session_attachments
          SET
            filename = ?,
            content_type = ?,
            size_bytes = ?,
            cloud_object_key = CASE
              WHEN session_attachments.sha256 = ?
                AND session_attachments.size_bytes = ? THEN cloud_object_key
              ELSE ''
            END,
            storage_kind = CASE
              WHEN session_attachments.sha256 = ?
                AND session_attachments.size_bytes = ? THEN storage_kind
              ELSE 'local_file'
            END,
            sha256 = ?,
            source_type = 'note_upload',
            source_id = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = NULL
          WHERE id = (
            SELECT attachment.id
            FROM session_attachments AS attachment
            JOIN sessions AS session
              ON session.id = attachment.session_id
              AND session.deleted_at IS NULL
            WHERE attachment.session_id = ?
              AND attachment.relative_path = ?
            ORDER BY attachment.deleted_at IS NULL DESC,
              attachment.updated_at DESC,
              attachment.id
            LIMIT 1
          )
        `,
        params: [
          filename,
          contentType,
          input.sizeBytes,
          input.sha256,
          input.sizeBytes,
          input.sha256,
          input.sizeBytes,
          input.sha256,
          attachmentId,
          sessionId,
          relativePath,
        ],
      },
      {
        sql: `
          INSERT INTO session_attachments (
            id,
            workspace_id,
            session_id,
            filename,
            relative_path,
            content_type,
            size_bytes,
            sha256,
            storage_kind,
            cloud_object_key,
            source_type,
            source_id,
            metadata_json
          )
          SELECT
            ?,
            session.workspace_id,
            session.id,
            ?,
            ?,
            ?,
            ?,
            ?,
            'local_file',
            '',
            'note_upload',
            ?,
            '{}'
          FROM sessions AS session
          WHERE session.id = ?
            AND session.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1
              FROM session_attachments AS attachment
              WHERE attachment.session_id = session.id
                AND attachment.relative_path = ?
                AND attachment.deleted_at IS NULL
            )
        `,
        params: [
          metadataId,
          filename,
          relativePath,
          contentType,
          input.sizeBytes,
          input.sha256,
          attachmentId,
          sessionId,
          relativePath,
        ],
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
          SELECT
            attachment.id,
            attachment.session_id,
            attachment.relative_path,
            'present',
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          FROM session_attachments AS attachment
          WHERE attachment.session_id = ?
            AND attachment.relative_path = ?
            AND attachment.deleted_at IS NULL
          ORDER BY attachment.updated_at DESC, attachment.id
          LIMIT 1
          ON CONFLICT(attachment_id) DO UPDATE SET
            session_id = excluded.session_id,
            relative_path = excluded.relative_path,
            availability = excluded.availability,
            updated_at = excluded.updated_at
        `,
        params: [sessionId, relativePath],
        expectedRowsAffected: 1,
      },
      enqueueAttachmentUploadStatement({
        jobId: uploadJobId,
        sessionId,
        attachmentId: null,
        relativePath,
      }),
    ]),
  );

  if ((results[1] ?? 0) + (results[2] ?? 0) !== 1 || (results[3] ?? 0) !== 1) {
    throw new Error("attachment session is unavailable");
  }
}

export async function catalogLocalSessionAudio(
  inputSessionId: string,
): Promise<void> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  await enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await fsSyncCommands.audioMetadata(sessionId);
    if (result.status === "error") {
      throw new Error(result.error);
    }
    if (!result.data) {
      throw new Error("audio_path_not_found");
    }

    const filename = requireBasename(result.data.filename, "audio filename");
    const contentType = requireText(
      result.data.contentType,
      "audio content type",
      512,
    );
    if (
      !Number.isSafeInteger(result.data.sizeBytes) ||
      result.data.sizeBytes < 0
    ) {
      throw new Error("invalid audio size");
    }
    if (!SHA256_PATTERN.test(result.data.sha256)) {
      throw new Error("invalid audio checksum");
    }

    const attachmentId = `session-audio:${sessionId}`;
    const deleteJobId = id();
    const uploadJobId = id();
    const results = await executeTransaction([
      enqueueReplacedAttachmentDeleteStatement({
        jobId: deleteJobId,
        sessionId,
        attachmentId,
        relativePath: null,
        nextSha256: result.data.sha256,
        nextSizeBytes: result.data.sizeBytes,
      }),
      {
        sql: `
          UPDATE session_attachments
          SET
            filename = ?,
            relative_path = ?,
            content_type = ?,
            size_bytes = ?,
            cloud_object_key = CASE
              WHEN session_attachments.sha256 = ?
                AND session_attachments.size_bytes = ? THEN cloud_object_key
              ELSE ''
            END,
            storage_kind = CASE
              WHEN session_attachments.sha256 = ?
                AND session_attachments.size_bytes = ? THEN storage_kind
              ELSE 'local_file'
            END,
            sha256 = ?,
            source_type = 'session_audio',
            source_id = 'primary',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = NULL
          WHERE id = ?
            AND session_id = ?
            AND EXISTS (
              SELECT 1
              FROM sessions AS session
              WHERE session.id = ?
                AND session.deleted_at IS NULL
            )
        `,
        params: [
          filename,
          filename,
          contentType,
          result.data.sizeBytes,
          result.data.sha256,
          result.data.sizeBytes,
          result.data.sha256,
          result.data.sizeBytes,
          result.data.sha256,
          attachmentId,
          sessionId,
          sessionId,
        ],
      },
      {
        sql: `
          INSERT INTO session_attachments (
            id,
            workspace_id,
            session_id,
            filename,
            relative_path,
            content_type,
            size_bytes,
            sha256,
            storage_kind,
            cloud_object_key,
            source_type,
            source_id,
            metadata_json
          )
          SELECT
            ?,
            session.workspace_id,
            session.id,
            ?,
            ?,
            ?,
            ?,
            ?,
            'local_file',
            '',
            'session_audio',
            'primary',
            '{}'
          FROM sessions AS session
          WHERE session.id = ?
            AND session.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1
              FROM session_attachments AS attachment
              WHERE attachment.id = ?
            )
        `,
        params: [
          attachmentId,
          filename,
          filename,
          contentType,
          result.data.sizeBytes,
          result.data.sha256,
          sessionId,
          attachmentId,
        ],
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
          SELECT
            attachment.id,
            attachment.session_id,
            attachment.relative_path,
            'present',
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          FROM session_attachments AS attachment
          WHERE attachment.id = ?
            AND attachment.session_id = ?
            AND attachment.deleted_at IS NULL
          ON CONFLICT(attachment_id) DO UPDATE SET
            session_id = excluded.session_id,
            relative_path = excluded.relative_path,
            availability = excluded.availability,
            updated_at = excluded.updated_at
        `,
        params: [attachmentId, sessionId],
        expectedRowsAffected: 1,
      },
      enqueueAttachmentUploadStatement({
        jobId: uploadJobId,
        sessionId,
        attachmentId,
        relativePath: null,
      }),
    ]);

    if (
      (results[1] ?? 0) + (results[2] ?? 0) !== 1 ||
      (results[3] ?? 0) !== 1
    ) {
      throw new Error("audio session is unavailable");
    }
  });
}

export async function setAttachmentCloudSyncEnabled(
  inputSessionId: string,
  inputAttachmentId: string,
  enabled: boolean,
): Promise<void> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  const attachmentId = requireText(inputAttachmentId, "attachment ID", 512);
  const now = new Date().toISOString();
  const statements = [
    {
      sql: `
        UPDATE session_attachments
        SET cloud_sync_enabled = ?, updated_at = ?
        WHERE id = ? AND session_id = ? AND deleted_at IS NULL
      `,
      params: [enabled ? 1 : 0, now, attachmentId, sessionId],
    },
    {
      sql: `
        UPDATE attachment_transfer_jobs
        SET phase = 'completed', completed_at = ?, updated_at = ?, last_error = ''
        WHERE attachment_id = ?
          AND direction IN (${enabled ? "'delete'" : "'upload', 'download'"})
          AND phase IN ('queued', 'retry_wait', 'failed')
      `,
      params: [now, now, attachmentId],
    },
  ];

  if (enabled) {
    statements.push(
      enqueueAttachmentUploadStatement({
        jobId: id(),
        sessionId,
        attachmentId,
        relativePath: null,
      }),
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
          SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
            'download', attachment.sha256, attachment.size_bytes,
            attachment.cloud_object_key
          FROM session_attachments AS attachment
          LEFT JOIN attachment_local_state AS local
            ON local.attachment_id = attachment.id
          WHERE attachment.id = ?
            AND attachment.session_id = ?
            AND attachment.cloud_sync_enabled = 1
            AND attachment.cloud_object_key <> ''
            AND attachment.deleted_at IS NULL
            AND COALESCE(local.availability, 'absent') <> 'present'
        `,
        params: [id(), attachmentId, sessionId],
      },
    );
  } else {
    statements.push(
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
          SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
            'download', attachment.sha256, attachment.size_bytes,
            attachment.cloud_object_key
          FROM session_attachments AS attachment
          LEFT JOIN attachment_local_state AS local
            ON local.attachment_id = attachment.id
          WHERE attachment.id = ?
            AND attachment.session_id = ?
            AND attachment.cloud_sync_enabled = 0
            AND attachment.cloud_object_key <> ''
            AND attachment.deleted_at IS NULL
            AND COALESCE(local.availability, 'absent') <> 'present'
        `,
        params: [id(), attachmentId, sessionId],
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
          SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
            'delete', attachment.sha256, attachment.size_bytes,
            attachment.cloud_object_key
          FROM session_attachments AS attachment
          JOIN attachment_local_state AS local
            ON local.attachment_id = attachment.id
            AND local.availability = 'present'
          WHERE attachment.id = ?
            AND attachment.session_id = ?
            AND attachment.cloud_sync_enabled = 0
            AND attachment.cloud_object_key <> ''
            AND attachment.deleted_at IS NULL
        `,
        params: [id(), attachmentId, sessionId],
      },
    );
  }

  const [updated = 0] = await enqueueDatabaseWrite(`session:${sessionId}`, () =>
    executeTransaction(statements),
  );
  if (updated !== 1) {
    throw new Error("attachment is unavailable");
  }
}

export async function deleteLocalSessionAudio(
  inputSessionId: string,
  canDelete: () => boolean,
): Promise<boolean> {
  return deleteSessionAudioWithMode(inputSessionId, false, canDelete);
}

export async function deleteSessionAudio(
  inputSessionId: string,
  canDelete: () => boolean,
): Promise<boolean> {
  return deleteSessionAudioWithMode(inputSessionId, true, canDelete);
}

export async function cleanupDeletedSessionAudio(
  inputSessionId: string,
  canDelete: () => boolean,
): Promise<boolean> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  return enqueueSessionAudioOperation(sessionId, () =>
    enqueueDatabaseWrite(`session:${sessionId}`, async () => {
      if (!canDelete()) {
        return false;
      }

      const rows = await liveQueryClient.execute<{ is_deleted: number }>(
        `
          SELECT EXISTS(
            SELECT 1
            FROM session_attachments
            WHERE id = ?
              AND session_id = ?
              AND deleted_at IS NOT NULL
              AND NOT EXISTS (
                SELECT 1
                FROM attachment_local_state AS local
                WHERE local.attachment_id = session_attachments.id
                  AND local.availability = 'absent'
              )
          ) AS is_deleted
        `,
        [`session-audio:${sessionId}`, sessionId],
      );
      if (rows[0]?.is_deleted !== 1) {
        return false;
      }

      return deleteSessionAudioFile(sessionId);
    }),
  );
}

async function deleteSessionAudioWithMode(
  inputSessionId: string,
  deleteMetadata: boolean,
  canDelete: () => boolean,
): Promise<boolean> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  return enqueueSessionAudioOperation(sessionId, () =>
    enqueueDatabaseWrite(`session:${sessionId}`, async () => {
      if (!canDelete()) {
        return false;
      }
      if (deleteMetadata) {
        await tombstoneSessionAudioMetadata(sessionId);
      }
      const deletedLocalFile = await deleteSessionAudioFile(sessionId);
      return deleteMetadata || deletedLocalFile;
    }),
  );
}

async function deleteSessionAudioFile(sessionId: string): Promise<boolean> {
  const result = await fsSyncCommands.audioDelete(sessionId);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  await markSessionAudioAvailability(sessionId, "absent");
  return result.data;
}

async function markSessionAudioAvailability(
  sessionId: string,
  availability: "present" | "absent",
): Promise<void> {
  await executeTransaction([
    {
      sql: `
        INSERT INTO attachment_local_state (
          attachment_id,
          session_id,
          relative_path,
          availability,
          updated_at
        ) VALUES (?, ?, '', ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ON CONFLICT(attachment_id) DO UPDATE SET
          session_id = excluded.session_id,
          availability = excluded.availability,
          updated_at = excluded.updated_at
      `,
      params: [`session-audio:${sessionId}`, sessionId, availability],
    },
  ]);
}

async function tombstoneSessionAudioMetadata(sessionId: string): Promise<void> {
  await executeTransaction([
    {
      sql: `
        UPDATE session_attachments
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
          AND session_id = ?
          AND deleted_at IS NULL
      `,
      params: [`session-audio:${sessionId}`, sessionId],
    },
  ]);
}

export async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

function enqueueReplacedAttachmentDeleteStatement(input: {
  jobId: string;
  sessionId: string;
  attachmentId: string | null;
  relativePath: string | null;
  nextSha256: string;
  nextSizeBytes: number;
}) {
  const selector = input.attachmentId
    ? "attachment.id = ?"
    : "attachment.relative_path = ?";
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
      )
      SELECT ?, attachment.id, attachment.session_id, attachment.workspace_id,
        'delete', attachment.sha256, attachment.size_bytes,
        attachment.cloud_object_key
      FROM session_attachments AS attachment
      WHERE attachment.session_id = ?
        AND ${selector}
        AND (attachment.sha256 <> ? OR attachment.size_bytes <> ?)
        AND attachment.cloud_object_key <> ''
      ORDER BY attachment.deleted_at IS NULL DESC,
        attachment.updated_at DESC,
        attachment.id
      LIMIT 1
    `,
    params: [
      input.jobId,
      input.sessionId,
      input.attachmentId ?? input.relativePath ?? "",
      input.nextSha256,
      input.nextSizeBytes,
    ],
  };
}

function enqueueAttachmentUploadStatement(input: {
  jobId: string;
  sessionId: string;
  attachmentId: string | null;
  relativePath: string | null;
}) {
  const selector = input.attachmentId
    ? "attachment.id = ?"
    : "attachment.relative_path = ?";
  return {
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
      WHERE attachment.session_id = ?
        AND ${selector}
        AND attachment.cloud_sync_enabled = 1
        AND attachment.cloud_object_key = ''
        AND attachment.deleted_at IS NULL
      ORDER BY attachment.updated_at DESC, attachment.id
      LIMIT 1
    `,
    params: [
      input.jobId,
      input.sessionId,
      input.attachmentId ?? input.relativePath ?? "",
    ],
  };
}

function requireBasename(value: unknown, label: string) {
  const basename = requireText(value, label, 1024);
  if (
    basename === "." ||
    basename === ".." ||
    basename.includes("/") ||
    basename.includes("\\") ||
    basename.includes("\0")
  ) {
    throw new Error(`invalid ${label}`);
  }
  return basename;
}

function requireText(
  value: unknown,
  label: string,
  maxLength: number,
  allowEmpty = false,
) {
  if (
    typeof value !== "string" ||
    (!allowEmpty && value.length === 0) ||
    value.length > maxLength ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/.test(value)
  ) {
    throw new Error(`invalid ${label}`);
  }
  return value;
}
