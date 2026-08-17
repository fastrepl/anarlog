export type SessionAudioRow = {
  id: string;
  filename: string;
  content_type: string;
  size_bytes: number;
  sha256: string;
  transcript_status: string;
  created_at: string;
  available_locally: number;
  local_relative_path: string;
  cloud_object_key: string;
};

export type SessionAudio = {
  attachmentId: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  transcriptStatus: string;
  createdAt: string;
  availableLocally: boolean;
  localRelativePath: string | null;
  cloudObjectKey: string | null;
};

export function mapSessionAudioRows(
  rows: SessionAudioRow[],
): SessionAudio | null {
  const row = rows[0];
  if (!row) return null;
  const availableLocally =
    row.available_locally === 1 && row.local_relative_path !== "";
  return {
    attachmentId: row.id,
    filename: row.filename,
    contentType: row.content_type,
    sizeBytes: row.size_bytes,
    sha256: row.sha256,
    transcriptStatus: row.transcript_status,
    createdAt: row.created_at,
    availableLocally,
    localRelativePath: availableLocally ? row.local_relative_path : null,
    cloudObjectKey: row.cloud_object_key || null,
  };
}
