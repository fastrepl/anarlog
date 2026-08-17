import assert from "node:assert/strict";
import test from "node:test";

import {
  assertRestorableAudioMetadata,
  assertRestoredAudioMatches,
  restoredAudioRelativePath,
} from "./audio-restore-model.ts";

const sha256 = "a".repeat(64);

test("accepts an exact recording match", () => {
  assert.doesNotThrow(() =>
    assertRestoredAudioMatches(
      { sha256, sizeBytes: 42 },
      { sha256, sizeBytes: 42 },
    ),
  );
});

test("rejects unverified legacy metadata", () => {
  assert.throws(
    () =>
      assertRestoredAudioMatches(
        { sha256: "", sizeBytes: 42 },
        { sha256, sizeBytes: 42 },
      ),
    /older recording cannot be verified/,
  );
});

test("rejects invalid synced audio sizes", () => {
  assert.throws(
    () => assertRestorableAudioMetadata({ sha256, sizeBytes: 0 }),
    /invalid synced file metadata/,
  );
});

test("rejects the wrong bytes or size", () => {
  assert.throws(
    () =>
      assertRestoredAudioMatches(
        { sha256, sizeBytes: 42 },
        { sha256: "b".repeat(64), sizeBytes: 42 },
      ),
    /does not match/,
  );
  assert.throws(
    () =>
      assertRestoredAudioMatches(
        { sha256, sizeBytes: 42 },
        { sha256, sizeBytes: 41 },
      ),
    /does not match/,
  );
});

test("derives only safe device-local audio filenames", () => {
  assert.equal(restoredAudioRelativePath("Meeting.M4A"), "restored-audio.m4a");
  assert.equal(
    restoredAudioRelativePath("../../another-session/audio.wav"),
    "restored-audio.wav",
  );
  assert.equal(
    restoredAudioRelativePath("recording.command"),
    "restored-audio.bin",
  );
});
