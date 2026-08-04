import assert from "node:assert/strict";
import test from "node:test";

import { createTrackedTimers } from "./tracked-timers.ts";

test("completed and cleared timers leave the registry", async () => {
  const timers = createTrackedTimers();
  await new Promise<void>((resolve) => {
    timers.setTimeout(resolve, 0);
  });
  assert.equal(timers.pendingCount(), 0);

  const interval = timers.setInterval(() => {}, 60_000);
  assert.equal(timers.pendingCount(), 1);
  timers.clearInterval(interval);
  assert.equal(timers.pendingCount(), 0);
});

test("clear releases every pending timer", () => {
  const timers = createTrackedTimers();
  for (let index = 0; index < 100; index += 1) {
    timers.setTimeout(() => {}, 60_000);
    timers.setInterval(() => {}, 60_000);
  }

  assert.equal(timers.pendingCount(), 200);
  timers.clear();
  assert.equal(timers.pendingCount(), 0);
});
