import assert from "node:assert/strict";
import test from "node:test";

import { recorderStatusFailure } from "./recorder-status.ts";

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
