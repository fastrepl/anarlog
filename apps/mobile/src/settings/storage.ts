import { useLiveQuery } from "@/db";

export const STORAGE_SQL = `
SELECT
  COUNT(*) AS recording_count,
  COALESCE(SUM(CASE WHEN local.availability = 'present' THEN attachment.size_bytes ELSE 0 END), 0) AS local_bytes,
  COALESCE(SUM(CASE WHEN local.availability = 'present' THEN 1 ELSE 0 END), 0) AS local_count,
  COALESCE(SUM(CASE WHEN local.availability = 'present' AND attachment.cloud_sync_enabled = 1
    AND attachment.cloud_object_key = '' THEN 1 ELSE 0 END), 0) AS pending_count
FROM session_attachments AS attachment
JOIN sessions AS session ON session.id = attachment.session_id AND session.deleted_at IS NULL
LEFT JOIN attachment_local_state AS local ON local.attachment_id = attachment.id
WHERE attachment.source_type = 'session_audio' AND attachment.deleted_at IS NULL
`;

export function useRecordingStorage() {
  return useLiveQuery<
    {
      recording_count: number;
      local_bytes: number;
      local_count: number;
      pending_count: number;
    },
    {
      recording_count: number;
      local_bytes: number;
      local_count: number;
      pending_count: number;
    }[]
  >({ sql: STORAGE_SQL, mapRows: (rows) => rows });
}

export function formatStorageBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
