import { executeTransaction } from "~/db";
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
            sha256 = ?,
            storage_kind = 'local_file',
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
    ]),
  );

  if ((results[0] ?? 0) + (results[1] ?? 0) !== 1) {
    throw new Error("attachment session is unavailable");
  }
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
