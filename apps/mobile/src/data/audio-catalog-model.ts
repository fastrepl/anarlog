export type SessionAudioRow = {
  filename: string;
  size_bytes: number;
  transcript_status: string;
  created_at: string;
  available_locally: number;
  local_relative_path: string;
};

export type SessionAudio = {
  filename: string;
  sizeBytes: number;
  transcriptStatus: string;
  createdAt: string;
  availableLocally: boolean;
  localRelativePath: string | null;
};

export function mapSessionAudioRows(
  rows: SessionAudioRow[],
): SessionAudio | null {
  const row = rows[0];
  if (!row) return null;
  const availableLocally =
    row.available_locally === 1 && row.local_relative_path !== "";
  return {
    filename: row.filename,
    sizeBytes: row.size_bytes,
    transcriptStatus: row.transcript_status,
    createdAt: row.created_at,
    availableLocally,
    localRelativePath: availableLocally ? row.local_relative_path : null,
  };
}
