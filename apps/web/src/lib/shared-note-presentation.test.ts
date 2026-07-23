import assert from "node:assert/strict";
import test from "node:test";

import {
  createSharedNoteWaveform,
  findFeaturedSharedNoteAudio,
  formatSharedNotePlaybackTime,
} from "./shared-note-presentation.ts";

test("finds the first playable shared audio attachment", () => {
  const audio = {
    id: "audio",
    filename: "meeting.m4a",
    contentType: "audio/mp4",
    sizeBytes: 10,
    sha256: "a".repeat(64),
  };
  assert.equal(
    findFeaturedSharedNoteAudio([
      { ...audio, id: "image", contentType: "image/png" },
      audio,
      { ...audio, id: "second" },
    ]),
    audio,
  );
});

test("builds a stable bounded waveform", () => {
  const first = createSharedNoteWaveform("abc", 12);
  assert.deepEqual(first, createSharedNoteWaveform("abc", 12));
  assert.equal(first.length, 12);
  assert.ok(first.every((height) => height >= 18 && height <= 90));
  assert.notDeepEqual(first, createSharedNoteWaveform("def", 12));
});

test("formats playback time without leaking invalid values", () => {
  assert.equal(formatSharedNotePlaybackTime(0), "0:00");
  assert.equal(formatSharedNotePlaybackTime(754.9), "12:34");
  assert.equal(formatSharedNotePlaybackTime(Number.NaN), "0:00");
});
