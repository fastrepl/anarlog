import assert from "node:assert/strict";
import test from "node:test";

import { isRecordingStartCancelled } from "./recorder-status.ts";

test("recognizes a native stop during recording startup as cancellation", () => {
  const error = Object.assign(new Error("Audio stream startup was cancelled"), {
    code: "ERR_AUDIO_STREAM_START_CANCELLED",
  });
  assert.equal(isRecordingStartCancelled(error), true);
});

test("keeps initialization and permission failures on the error path", () => {
  for (const error of [
    { code: "ERR_AUDIO_STREAM_INITIALIZATION" },
    { code: "ERR_AUDIO_RECORDING_SERVICE" },
    { code: "ERR_AUDIO_PERMISSIONS" },
    new Error("Audio stream startup was cancelled"),
    null,
    undefined,
  ]) {
    assert.equal(isRecordingStartCancelled(error), false);
  }
});
