import { getDocumentAsync } from "expo-document-picker";
import { Directory, File, FileMode, Paths } from "expo-file-system";

import { requestMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
import { hashFileSha256 } from "@/data/file-sha256";
import {
  portableNoteAttachmentMarkdown,
  requireNoteAttachmentFilename,
} from "@/data/note-attachment-model";
import { executeTransaction } from "@/db";
import { id, nowIso } from "@/lib/ids";

const MAX_ATTACHMENT_BYTES = 512 * 1024 * 1024;
const UUID_V4_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export async function pickAndCatalogNoteAttachment(
  sessionId: string,
  signal?: AbortSignal,
): Promise<
  | { status: "cancelled" }
  | {
      status: "attached";
      attachmentId: string;
      filename: string;
      markdown: string;
    }
> {
  if (!UUID_V4_PATTERN.test(sessionId)) {
    throw new Error("The meeting identifier is invalid.");
  }
  const result = await getDocumentAsync({
    type: "*/*",
    multiple: false,
    copyToCacheDirectory: true,
  });
  if (result.canceled) return { status: "cancelled" };
  throwIfAborted(signal);

  const asset = result.assets[0];
  const filename = requireNoteAttachmentFilename(asset.name);
  if (typeof asset.size === "number" && asset.size > MAX_ATTACHMENT_BYTES) {
    throw new Error("Attachments must be 512 MB or smaller.");
  }
  const attachmentId = id();
  const relativePath = `attachments/${attachmentId}`;
  const directory = new Directory(
    Paths.document,
    "sessions",
    sessionId,
    "attachments",
  );
  directory.create({ intermediates: true, idempotent: true });
  const destination = new File(directory, attachmentId);

  try {
    await new File(asset.uri).copy(destination);
    const verified = await hashFileSha256(
      { open: () => destination.open(FileMode.ReadOnly) },
      signal,
    );
    if (verified.sizeBytes > MAX_ATTACHMENT_BYTES) {
      throw new Error("Attachments must be 512 MB or smaller.");
    }
    const contentType = validContentType(asset.mimeType)
      ? asset.mimeType
      : "application/octet-stream";
    const now = nowIso();
    const transferJobId = id();
    const [inserted = 0, localized = 0, queued = 0] = await executeTransaction([
      {
        sql: `
            INSERT INTO session_attachments (
              id, workspace_id, session_id, filename, relative_path,
              content_type, size_bytes, sha256, storage_kind,
              cloud_object_key, source_type, source_id, metadata_json,
              cloud_sync_enabled, created_at, updated_at, deleted_at
            )
            SELECT ?, workspace_id, id, ?, ?, ?, ?, ?, 'local_file', '',
              'note_upload', ?, '{}', 1, ?, ?, NULL
            FROM sessions
            WHERE id = ? AND deleted_at IS NULL
          `,
        params: [
          attachmentId,
          filename,
          relativePath,
          contentType,
          verified.sizeBytes,
          verified.sha256,
          attachmentId,
          now,
          now,
          sessionId,
        ],
        expectedRowsAffected: 1,
      },
      {
        sql: `
            INSERT INTO attachment_local_state (
              attachment_id, session_id, relative_path, availability, updated_at
            )
            SELECT id, session_id, relative_path, 'present', ?
            FROM session_attachments
            WHERE id = ? AND session_id = ? AND deleted_at IS NULL
          `,
        params: [now, attachmentId, sessionId],
        expectedRowsAffected: 1,
      },
      {
        sql: `
            INSERT INTO attachment_transfer_jobs (
              id, attachment_id, session_id, workspace_id, direction,
              expected_sha256, expected_size_bytes
            )
            SELECT ?, id, session_id, workspace_id, 'upload', sha256, size_bytes
            FROM session_attachments
            WHERE id = ? AND session_id = ? AND cloud_sync_enabled = 1
              AND deleted_at IS NULL
          `,
        params: [transferJobId, attachmentId, sessionId],
        expectedRowsAffected: 1,
      },
    ]);
    if (inserted !== 1 || localized !== 1 || queued !== 1) {
      throw new Error("The attachment could not be added to this meeting.");
    }
    requestMobileAttachmentUploads();
    return {
      status: "attached",
      attachmentId,
      filename,
      markdown: portableNoteAttachmentMarkdown(attachmentId, filename),
    };
  } catch (error) {
    try {
      if (destination.exists) destination.delete();
    } catch {}
    throw error;
  }
}

function validContentType(value: string | null | undefined): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 512 &&
    value.trim() === value &&
    !hasControlCharacter(value)
  );
}

function hasControlCharacter(value: string): boolean {
  return [...value].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 31 || code === 127;
  });
}

function throwIfAborted(signal?: AbortSignal): void {
  if (!signal?.aborted) return;
  throw signal.reason instanceof Error
    ? signal.reason
    : new Error("Attachment import was cancelled.");
}
