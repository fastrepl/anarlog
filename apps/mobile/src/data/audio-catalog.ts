import { File, FileMode, Paths } from "expo-file-system";

import { requestMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
import { requeueMobileAttachmentUpload } from "@/attachment-sync/upload-store";
import { executeTransaction, useLiveQuery } from "@/db";
import { id, nowIso } from "@/lib/ids";

import {
  mapSessionAudioRows,
  type SessionAudio,
  type SessionAudioRow,
} from "./audio-catalog-model";
import { hashFileSha256 } from "./file-sha256";

export type { SessionAudio } from "./audio-catalog-model";

const ATTACHMENT_UPSERT_SQL = `
INSERT INTO session_attachments (
  id, workspace_id, session_id, filename, relative_path, content_type,
  size_bytes, sha256, storage_kind, cloud_object_key, source_type,
  source_id, metadata_json, cloud_sync_enabled, created_at, updated_at, deleted_at
)
SELECT ?, workspace_id, id, ?, ?, ?, ?, ?, 'local_file', '',
  'session_audio', 'primary', json_object('transcript_status', 'processing', 'capture_started_at_ms', CASE WHEN ? THEN json_extract(metadata_json, '$.mobile_capture_started_at_ms') ELSE NULL END, 'capture_interrupted_at_ms', CASE WHEN ? THEN json_extract(metadata_json, '$.mobile_capture_interrupted_at_ms') ELSE NULL END), 1, ?, ?, NULL
FROM sessions
WHERE id = ? AND deleted_at IS NULL
ON CONFLICT(id) DO UPDATE SET
  filename = excluded.filename,
  relative_path = excluded.relative_path,
  content_type = excluded.content_type,
  size_bytes = excluded.size_bytes,
  sha256 = excluded.sha256,
  storage_kind = 'local_file',
  cloud_object_key = '',
  cloud_sync_enabled = excluded.cloud_sync_enabled,
  metadata_json = excluded.metadata_json,
  updated_at = excluded.updated_at,
  deleted_at = NULL
`;

// Mirrors the attachment upsert's guard: without it a missing or deleted session
// skips the attachment insert but still commits local state pointing at nothing.
const LOCAL_STATE_UPSERT_SQL = `
INSERT INTO attachment_local_state (
  attachment_id, session_id, relative_path, availability, updated_at
)
SELECT ?, ?, ?, 'present', ?
WHERE EXISTS (
  SELECT 1 FROM session_attachments WHERE id = ? AND deleted_at IS NULL
)
ON CONFLICT(attachment_id) DO UPDATE SET
  session_id = excluded.session_id,
  relative_path = excluded.relative_path,
  availability = 'present',
  updated_at = excluded.updated_at
`;

export async function recordSessionCaptureStart(sessionId: string) {
  await executeTransaction([
    {
      sql: "UPDATE sessions SET metadata_json = json_remove(json_set(metadata_json, '$.mobile_capture_started_at_ms', ?), '$.mobile_capture_interrupted_at_ms'), updated_at = ? WHERE id = ? AND deleted_at IS NULL",
      params: [Date.now(), nowIso(), sessionId],
    },
  ]);
}

export async function recordSessionCaptureInterruption(sessionId: string) {
  await executeTransaction([
    {
      sql: "UPDATE sessions SET metadata_json = json_set(metadata_json, '$.mobile_capture_interrupted_at_ms', ?), updated_at = ? WHERE id = ? AND deleted_at IS NULL",
      params: [Date.now(), nowIso(), sessionId],
    },
  ]);
}

export async function catalogSessionAudio(
  sessionId: string,
  file: {
    source: "recording" | "import";
    filename: string;
    contentType: string;
    signal?: AbortSignal;
  },
): Promise<void> {
  const attachmentId = `session-audio:${sessionId}`;
  const now = nowIso();
  const localFile = new File(
    Paths.document,
    "sessions",
    sessionId,
    file.filename,
  );
  const verified = await hashFileSha256(
    {
      open: () => localFile.open(FileMode.ReadOnly),
    },
    file.signal,
  );

  await executeTransaction([
    {
      sql: ATTACHMENT_UPSERT_SQL,
      params: [
        attachmentId,
        file.filename,
        file.filename,
        file.contentType,
        verified.sizeBytes,
        verified.sha256,
        file.source === "recording" ? 1 : 0,
        file.source === "recording" ? 1 : 0,
        now,
        now,
        sessionId,
      ],
    },
    {
      sql: LOCAL_STATE_UPSERT_SQL,
      params: [attachmentId, sessionId, file.filename, now, attachmentId],
    },
  ]);
}

export async function enableSessionAudioBackup(
  attachmentId: string,
): Promise<void> {
  const now = nowIso();
  const transferJobId = id();
  await executeTransaction([
    {
      sql: `
        UPDATE session_attachments
        SET cloud_sync_enabled = 1, updated_at = ?
        WHERE id = ? AND source_type = 'session_audio' AND deleted_at IS NULL
          AND workspace_id <> ''
          AND EXISTS (
            SELECT 1 FROM attachment_local_state
            WHERE attachment_id = ? AND availability = 'present'
          )
      `,
      params: [now, attachmentId, attachmentId],
      expectedRowsAffected: 1,
    },
    {
      sql: `
        INSERT OR IGNORE INTO attachment_transfer_jobs (
          id, attachment_id, session_id, workspace_id, direction,
          expected_sha256, expected_size_bytes
        )
        SELECT ?, id, session_id, workspace_id, 'upload', sha256, size_bytes
        FROM session_attachments
        WHERE id = ? AND deleted_at IS NULL AND cloud_sync_enabled = 1
          AND cloud_object_key = ''
          AND EXISTS (
            SELECT 1 FROM attachment_local_state
            WHERE attachment_id = ? AND availability = 'present'
          )
      `,
      params: [transferJobId, attachmentId, attachmentId],
    },
  ]);
  await requeueMobileAttachmentUpload(attachmentId);
  requestMobileAttachmentUploads();
}

const SESSION_AUDIO_SQL = `
SELECT
  attachment.id,
  attachment.workspace_id,
  attachment.filename,
  attachment.content_type,
  attachment.size_bytes,
  attachment.sha256,
  COALESCE(json_extract(attachment.metadata_json, '$.transcript_status'), '') AS transcript_status,
  CAST(json_extract(attachment.metadata_json, '$.capture_started_at_ms') AS INTEGER) AS capture_started_at_ms,
  CAST(json_extract(attachment.metadata_json, '$.capture_interrupted_at_ms') AS INTEGER) AS capture_interrupted_at_ms,
  attachment.created_at,
  CASE WHEN local_state.availability = 'present' THEN 1 ELSE 0 END AS available_locally,
  COALESCE(local_state.relative_path, '') AS local_relative_path,
  attachment.cloud_object_key,
  attachment.cloud_sync_enabled,
  upload.phase AS upload_phase,
  COALESCE(upload.last_error, '') AS upload_error
FROM session_attachments AS attachment
LEFT JOIN attachment_local_state AS local_state
  ON local_state.attachment_id = attachment.id
LEFT JOIN attachment_transfer_jobs AS upload
  ON upload.id = (
    SELECT candidate.id
    FROM attachment_transfer_jobs AS candidate
    WHERE candidate.attachment_id = attachment.id
      AND candidate.direction = 'upload'
      AND candidate.expected_sha256 = attachment.sha256
      AND candidate.expected_size_bytes = attachment.size_bytes
    ORDER BY candidate.updated_at DESC, candidate.id DESC
    LIMIT 1
  )
WHERE attachment.session_id = ?
  AND attachment.source_type = 'session_audio'
  AND attachment.deleted_at IS NULL
LIMIT 1
`;

export function useSessionAudio(sessionId: string): {
  data: SessionAudio | null;
  isLoading: boolean;
} {
  const { data, isLoading } = useLiveQuery<
    SessionAudioRow,
    SessionAudio | null
  >({
    sql: SESSION_AUDIO_SQL,
    params: [sessionId],
    mapRows: mapSessionAudioRows,
  });
  return { data: data ?? null, isLoading };
}

export async function retrySessionAudioUpload(
  attachmentId: string,
): Promise<void> {
  await requeueMobileAttachmentUpload(attachmentId);
  requestMobileAttachmentUploads();
}
