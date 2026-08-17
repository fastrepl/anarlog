import assert from "node:assert/strict";
import test from "node:test";

import { mapSessionAudioRows } from "./audio-catalog-model.ts";

const row = {
  filename: "audio.wav",
  size_bytes: 42,
  transcript_status: "complete",
  created_at: "2026-08-17T00:00:00.000Z",
  available_locally: 1,
  local_relative_path: "audio.wav",
};

test("maps a device-local audio attachment to a playable file", () => {
  assert.deepEqual(mapSessionAudioRows([row]), {
    filename: "audio.wav",
    sizeBytes: 42,
    transcriptStatus: "complete",
    createdAt: "2026-08-17T00:00:00.000Z",
    availableLocally: true,
    localRelativePath: "audio.wav",
  });
});

test("does not treat synced metadata as a local audio file", () => {
  assert.deepEqual(
    mapSessionAudioRows([
      { ...row, available_locally: 0, local_relative_path: "" },
    ]),
    {
      filename: "audio.wav",
      sizeBytes: 42,
      transcriptStatus: "complete",
      createdAt: "2026-08-17T00:00:00.000Z",
      availableLocally: false,
      localRelativePath: null,
    },
  );
});

test("requires a local relative path before exposing playback", () => {
  const audio = mapSessionAudioRows([
    { ...row, available_locally: 1, local_relative_path: "" },
  ]);

  assert.equal(audio?.availableLocally, false);
  assert.equal(audio?.localRelativePath, null);
});
