export type AttachmentTransferDirection = "upload" | "download" | "delete";
export type AttachmentTransferPhase =
  | "queued"
  | "preparing"
  | "ready"
  | "transferring"
  | "finalizing"
  | "retry_wait"
  | "failed"
  | "completed";

export type AttachmentTransferJob = {
  id: string;
  attachmentId: string;
  sessionId: string;
  workspaceId: string;
  direction: AttachmentTransferDirection;
  expectedSha256: string;
  expectedSizeBytes: number;
  ciphertextSha256: string;
  ciphertextSizeBytes: number;
  remoteObjectId: string;
  objectKey: string;
  cacheId: string;
  phase: AttachmentTransferPhase;
  attemptCount: number;
  cloudSyncEnabled: boolean;
  currentObjectKey: string;
  attachmentDeleted: boolean;
  localAvailability: "present" | "absent";
  attachmentVersionMatches: boolean;
};

export type ReconcileRow = {
  id: string;
  session_id: string;
  workspace_id: string;
  sha256: string;
  size_bytes: number;
  cloud_object_key: string;
  cloud_sync_enabled: number | boolean;
  deleted_at: string | null;
  local_availability: string;
};

export type JobRow = {
  id: string;
  attachment_id: string;
  session_id: string;
  workspace_id: string;
  direction: string;
  expected_sha256: string;
  expected_size_bytes: number;
  ciphertext_sha256: string;
  ciphertext_size_bytes: number;
  remote_object_id: string;
  object_key: string;
  cache_id: string;
  phase: string;
  attempt_count: number;
  cloud_sync_enabled: number | boolean | null;
  current_object_key: string | null;
  attachment_deleted: number | boolean | null;
  local_availability: string | null;
  attachment_version_matches: number | boolean | null;
};

export type ObsoleteDownloadJobRow = {
  id: string;
  attempt_count: number;
};

export const ACTIVE_PHASES =
  "'preparing', 'ready', 'transferring', 'finalizing'";
