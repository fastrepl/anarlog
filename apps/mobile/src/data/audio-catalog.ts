import { File, FileMode, Paths } from "expo-file-system";

import { executeTransaction, useLiveQuery } from "@/db";
import { nowIso } from "@/lib/ids";

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
  source_id, metadata_json, created_at, updated_at, deleted_at
)
SELECT ?, workspace_id, id, ?, ?, ?, ?, ?, 'local_file', '',
  'session_audio', 'primary', '{"transcript_status":"processing"}', ?, ?, NULL
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

export async function catalogSessionAudio(
  sessionId: string,
  file: {
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

const SESSION_AUDIO_SQL = `
SELECT
  attachment.filename,
  attachment.size_bytes,
  COALESCE(json_extract(attachment.metadata_json, '$.transcript_status'), '') AS transcript_status,
  attachment.created_at,
  CASE WHEN local_state.availability = 'present' THEN 1 ELSE 0 END AS available_locally,
  COALESCE(local_state.relative_path, '') AS local_relative_path
FROM session_attachments AS attachment
LEFT JOIN attachment_local_state AS local_state
  ON local_state.attachment_id = attachment.id
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
