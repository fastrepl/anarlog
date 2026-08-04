import assert from "node:assert/strict";
import test from "node:test";

import {
  createClarityFallback,
  disableClarity,
  MAX_QUEUED_CLARITY_OPERATIONS,
} from "./clarity-queue.ts";

test("fallback retains only the latest bounded operations", () => {
  const clarity = createClarityFallback();
  for (let index = 0; index < 100; index += 1) {
    clarity("event", index);
  }

  assert.equal(clarity.q?.length, MAX_QUEUED_CLARITY_OPERATIONS);
  assert.equal(clarity.q?.[0]?.[1], 100 - MAX_QUEUED_CLARITY_OPERATIONS);
});

test("disabling a blocked fallback discards prior analytics work", () => {
  const clarity = createClarityFallback();
  for (let index = 0; index < 20; index += 1) {
    clarity("event", index);
  }

  disableClarity(clarity);

  assert.equal(clarity.q?.length, 2);
  assert.equal(clarity.q?.[0]?.[0], "consentv2");
  assert.equal(clarity.q?.[1]?.[0], "stop");
});
