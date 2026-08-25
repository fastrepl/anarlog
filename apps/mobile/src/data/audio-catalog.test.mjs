import assert from "node:assert/strict";
import test from "node:test";

import { mapSessionAudioRows } from "./audio-catalog-model.ts";

const row = {
  id: "session-audio:session-1",
  filename: "audio.wav",
  content_type: "audio/wav",
  size_bytes: 42,
  sha256: "a".repeat(64),
  transcript_status: "complete",
  created_at: "2026-08-17T00:00:00.000Z",
  available_locally: 1,
  local_relative_path: "audio.wav",
  cloud_object_key:
    "e8d8149f-af6a-4c14-8b91-066fa196187c/02425e17-c452-41c9-869c-196d09f75c91.anb1",
  upload_phase: "completed",
  upload_error: "",
};

test("maps a device-local audio attachment to a playable file", () => {
  assert.deepEqual(mapSessionAudioRows([row]), {
    attachmentId: "session-audio:session-1",
    filename: "audio.wav",
    contentType: "audio/wav",
    sizeBytes: 42,
    sha256: "a".repeat(64),
    transcriptStatus: "complete",
    createdAt: "2026-08-17T00:00:00.000Z",
    availableLocally: true,
    localRelativePath: "audio.wav",
    cloudObjectKey:
      "e8d8149f-af6a-4c14-8b91-066fa196187c/02425e17-c452-41c9-869c-196d09f75c91.anb1",
    deliveryState: "synced",
    uploadPhase: "completed",
    uploadError: null,
  });
});

test("does not treat synced metadata as a local audio file", () => {
  assert.deepEqual(
    mapSessionAudioRows([
      { ...row, available_locally: 0, local_relative_path: "" },
    ]),
    {
      attachmentId: "session-audio:session-1",
      filename: "audio.wav",
      contentType: "audio/wav",
      sizeBytes: 42,
      sha256: "a".repeat(64),
      transcriptStatus: "complete",
      createdAt: "2026-08-17T00:00:00.000Z",
      availableLocally: false,
      localRelativePath: null,
      cloudObjectKey:
        "e8d8149f-af6a-4c14-8b91-066fa196187c/02425e17-c452-41c9-869c-196d09f75c91.anb1",
      deliveryState: "synced",
      uploadPhase: "completed",
      uploadError: null,
    },
  );
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

test("requires a local relative path before exposing playback", () => {
  const audio = mapSessionAudioRows([
    { ...row, available_locally: 1, local_relative_path: "" },
  ]);

  assert.equal(audio?.availableLocally, false);
  assert.equal(audio?.localRelativePath, null);
});
