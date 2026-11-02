import assert from "node:assert/strict";
import test from "node:test";

import { mapSessionAudioRows } from "./audio-catalog-model.ts";

const row = {
  id: "session-audio:session-1",
  workspace_id: "workspace-1",
  filename: "audio.wav",
  content_type: "audio/wav",
  size_bytes: 42,
  sha256: "a".repeat(64),
  transcript_status: "complete",
  capture_started_at_ms: 1_000,
  capture_interrupted_at_ms: null,
  created_at: "2026-08-17T00:00:00.000Z",
  available_locally: 1,
  local_relative_path: "audio.wav",
  cloud_object_key:
    "e8d8149f-af6a-4c14-8b91-066fa196187c/02425e17-c452-41c9-869c-196d09f75c91.anb1",
  cloud_sync_enabled: 1,
  upload_phase: "completed",
  upload_error: "",
};

test("maps a device-local audio attachment to a playable file", () => {
  assert.deepEqual(mapSessionAudioRows([row]), {
    attachmentId: "session-audio:session-1",
    workspaceId: "workspace-1",
    filename: "audio.wav",
    contentType: "audio/wav",
    sizeBytes: 42,
    sha256: "a".repeat(64),
    transcriptStatus: "complete",
    captureStartedAtMs: 1_000,
    captureInterruptedAtMs: null,
    createdAt: "2026-08-17T00:00:00.000Z",
    availableLocally: true,
    localRelativePath: "audio.wav",
    cloudObjectKey:
      "e8d8149f-af6a-4c14-8b91-066fa196187c/02425e17-c452-41c9-869c-196d09f75c91.anb1",
    cloudSyncEnabled: true,
    deliveryState: "synced",
    uploadPhase: "completed",
    uploadError: null,
  });
});

test("does not treat synced metadata as a local audio file", () => {
  const synced = mapSessionAudioRows([
    { ...row, available_locally: 0, local_relative_path: "" },
  ]);
  assert.equal(synced?.availableLocally, false);
  assert.equal(synced?.localRelativePath, null);
  assert.equal(synced?.deliveryState, "synced");

  const missingPath = mapSessionAudioRows([
    { ...row, available_locally: 1, local_relative_path: "" },
  ]);
  assert.equal(missingPath?.availableLocally, false);
  assert.equal(missingPath?.localRelativePath, null);
});

test("maps the durable upload job to a per-record delivery state", () => {
  assert.equal(
    mapSessionAudioRows([
      {
        ...row,
        cloud_object_key: "",
        upload_phase: "transferring",
      },
    ])?.deliveryState,
    "uploading",
  );
  assert.equal(
    mapSessionAudioRows([
      { ...row, cloud_object_key: "", upload_phase: "failed" },
    ])?.deliveryState,
    "failed",
  );
});

test("keeps audio local when cloud backup has not been enabled", () => {
  const local = mapSessionAudioRows([
    { ...row, cloud_object_key: "", cloud_sync_enabled: 0, upload_phase: null },
  ]);
  assert.equal(local?.cloudSyncEnabled, false);
  assert.equal(local?.deliveryState, "local");
});
