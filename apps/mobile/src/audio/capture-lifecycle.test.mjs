import assert from "node:assert/strict";
import test from "node:test";

import {
  beginMobileCapture,
  endMobileCapture,
  getMobileCaptureActive,
} from "./capture-lifecycle.ts";

test("keeps the mobile gate open until every active capture is settled", () => {
  beginMobileCapture("session-1");
  beginMobileCapture("session-2");
  assert.equal(getMobileCaptureActive(), true);

  endMobileCapture("session-1");
  assert.equal(getMobileCaptureActive(), true);

  endMobileCapture("session-2");
  assert.equal(getMobileCaptureActive(), false);
});
