import assert from "node:assert/strict";
import test from "node:test";

import {
  recoverableRecordingUri,
  recorderRecoveryAction,
  recorderStatusFailure,
  shouldHandleRecorderFailure,
} from "./recorder-status.ts";

test("falls back to the recorder URI when native status omits it", () => {
  assert.equal(
    recoverableRecordingUri(null, "file:///recording.m4a"),
    "file:///recording.m4a",
  );
  assert.equal(
    recoverableRecordingUri("file:///status.m4a", "file:///recording.m4a"),
    "file:///status.m4a",
  );
});

test("persists recoverable audio before handling stop or retry", () => {
  for (const phase of ["interrupted", "save_error", "error"]) {
    assert.equal(recorderRecoveryAction(phase, true, "stop"), "persist");
    assert.equal(recorderRecoveryAction(phase, true, "retry"), "persist");
  }
});

test("retries a native stop when a save error has no recoverable URI", () => {
  assert.equal(recorderRecoveryAction("save_error", false, "retry"), "stop");
});

test("restarts capture for recoverable recorder failures without audio", () => {
  assert.equal(
    recorderRecoveryAction("interrupted", false, "retry"),
    "restart",
  );
  assert.equal(recorderRecoveryAction("error", false, "retry"), "restart");
  assert.equal(
    recorderRecoveryAction("unavailable", false, "retry"),
    "restart",
  );
});

test("leaves settled recorder phases alone", () => {
  assert.equal(recorderRecoveryAction("idle", false, "stop"), "noop");
  assert.equal(recorderRecoveryAction("saved", false, "retry"), "noop");
});

test("ignores delayed recorder failures after capture has settled", () => {
  for (const phase of ["idle", "saving", "saved", "unavailable"]) {
    assert.equal(shouldHandleRecorderFailure(phase), false);
  }
  for (const phase of [
    "starting",
    "recording",
    "interrupted",
    "save_error",
    "error",
  ]) {
    assert.equal(shouldHandleRecorderFailure(phase), true);
  }
});

test("classifies a media-service reset as a recoverable interruption", () => {
  assert.deepEqual(
    recorderStatusFailure({
      error: null,
      hasError: false,
      mediaServicesDidReset: true,
    }),
    {
      phase: "interrupted",
      reason: "media_services_reset",
      message: "Audio media services reset",
    },
  );
});

test("classifies native recorder failures without exposing an empty message", () => {
  assert.deepEqual(recorderStatusFailure({ error: null, hasError: true }), {
    phase: "error",
    reason: "native_error",
    message: "Native recording failed",
  });
  assert.equal(recorderStatusFailure({ error: null, hasError: false }), null);
});
