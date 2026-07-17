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
  const results = await enqueueDatabaseWrite(`session:${sessionId}`, () =>
    executeTransaction([
      {
        sql: `
          UPDATE session_attachments
          SET
            filename = ?,
            content_type = ?,
            size_bytes = ?,
            cloud_object_key = CASE
              WHEN session_attachments.sha256 = ? THEN cloud_object_key
              ELSE ''
            END,
            storage_kind = CASE
              WHEN session_attachments.sha256 = ? THEN storage_kind
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
          input.sha256,
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
    ]),
  );

  if ((results[0] ?? 0) + (results[1] ?? 0) !== 1 || (results[2] ?? 0) !== 1) {
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
    const results = await executeTransaction([
      {
        sql: `
          UPDATE session_attachments
          SET
            filename = ?,
            relative_path = ?,
            content_type = ?,
            size_bytes = ?,
            cloud_object_key = CASE
              WHEN session_attachments.sha256 = ? THEN cloud_object_key
              ELSE ''
            END,
            storage_kind = CASE
              WHEN session_attachments.sha256 = ? THEN storage_kind
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
          result.data.sha256,
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
    ]);

    if (
      (results[0] ?? 0) + (results[1] ?? 0) !== 1 ||
      (results[2] ?? 0) !== 1
    ) {
      throw new Error("audio session is unavailable");
    }
  });
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
