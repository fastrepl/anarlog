import assert from "node:assert/strict";
import test from "node:test";

import {
  createSharedNoteWaveform,
  findFeaturedSharedNoteAudio,
  formatSharedNotePlaybackTime,
  formatSharedNoteRelativeTime,
  isSharedNoteAudioGrantExpiring,
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

test("formats shared note comment timestamps relative to a stable time", () => {
  const now = Date.parse("2026-07-23T12:00:00Z");
  assert.equal(
    formatSharedNoteRelativeTime("2026-07-23T11:59:30Z", now),
    "30 seconds ago",
  );
  assert.equal(
    formatSharedNoteRelativeTime("2026-07-22T12:00:00Z", now),
    "yesterday",
  );
});

test("refreshes audio grants before they expire", () => {
  const now = Date.parse("2026-07-23T12:00:00Z");
  assert.equal(
    isSharedNoteAudioGrantExpiring("2026-07-23T12:00:10Z", now),
    true,
  );
  assert.equal(
    isSharedNoteAudioGrantExpiring("2026-07-23T12:00:11Z", now),
    false,
  );
});
