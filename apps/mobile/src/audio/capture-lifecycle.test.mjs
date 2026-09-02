import assert from "node:assert/strict";
import test from "node:test";

import {
  beginMobileCapture,
  endMobileCapture,
  getMobileCaptureActive,
  stopMobileCapture,
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

test("stops the active capture through the registered handler", async () => {
  const stopped = [];
  beginMobileCapture("session-1", async () => {
    stopped.push("session-1");
    endMobileCapture("session-1");
  });

  assert.equal(await stopMobileCapture(), true);
  assert.deepEqual(stopped, ["session-1"]);
  assert.equal(getMobileCaptureActive(), false);
  assert.equal(await stopMobileCapture(), false);
});

test("waits for a starting capture to register its stop handler", async () => {
  beginMobileCapture("session-1");
  const stopRequested = stopMobileCapture();

  beginMobileCapture("session-1", async () => {
    endMobileCapture("session-1");
  });

  assert.equal(await stopRequested, true);
  assert.equal(getMobileCaptureActive(), false);
});
