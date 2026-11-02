export type SessionAudioRow = {
  id: string;
  workspace_id: string;
  filename: string;
  content_type: string;
  size_bytes: number;
  sha256: string;
  transcript_status: string;
  capture_started_at_ms: number | null;
  capture_interrupted_at_ms: number | null;
  created_at: string;
  available_locally: number;
  local_relative_path: string;
  cloud_object_key: string;
  cloud_sync_enabled: number | boolean;
  upload_phase: string | null;
  upload_error: string;
};

export type SessionAudioDeliveryState =
  | "local"
  | "queued"
  | "uploading"
  | "synced"
  | "failed";

export type SessionAudio = {
  attachmentId: string;
  workspaceId: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  transcriptStatus: string;
  captureStartedAtMs: number | null;
  captureInterruptedAtMs: number | null;
  createdAt: string;
  availableLocally: boolean;
  localRelativePath: string | null;
  cloudObjectKey: string | null;
  cloudSyncEnabled: boolean;
  deliveryState: SessionAudioDeliveryState;
  uploadPhase: string | null;
  uploadError: string | null;
};

export function mapSessionAudioRows(
  rows: SessionAudioRow[],
): SessionAudio | null {
  const row = rows[0];
  if (!row) return null;
  const availableLocally =
    row.available_locally === 1 && row.local_relative_path !== "";
  const cloudObjectKey = row.cloud_object_key || null;
  const deliveryState: SessionAudioDeliveryState = !row.cloud_sync_enabled
    ? "local"
    : cloudObjectKey
      ? "synced"
      : row.upload_phase === "failed"
        ? "failed"
        : ["preparing", "ready", "transferring", "finalizing"].includes(
              row.upload_phase ?? "",
            )
          ? "uploading"
          : "queued";
  return {
    attachmentId: row.id,
    workspaceId: row.workspace_id,
    filename: row.filename,
    contentType: row.content_type,
    sizeBytes: row.size_bytes,
    sha256: row.sha256,
    transcriptStatus: row.transcript_status,
    captureStartedAtMs: row.capture_started_at_ms,
    captureInterruptedAtMs: row.capture_interrupted_at_ms,
    createdAt: row.created_at,
    availableLocally,
    localRelativePath: availableLocally ? row.local_relative_path : null,
    cloudObjectKey,
    cloudSyncEnabled: Boolean(row.cloud_sync_enabled),
    deliveryState,
    uploadPhase: row.upload_phase,
    uploadError: row.upload_error || null,
  };
}
