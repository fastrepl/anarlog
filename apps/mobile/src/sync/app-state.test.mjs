import assert from "node:assert/strict";
import test from "node:test";

import { shouldSyncAfterAppStateChange } from "./app-state.ts";

test("requests a sync when the app returns to the foreground", () => {
  assert.equal(shouldSyncAfterAppStateChange("background", "active"), true);
  assert.equal(shouldSyncAfterAppStateChange("inactive", "active"), true);
});

test("does not request duplicate or background sync rounds", () => {
  assert.equal(shouldSyncAfterAppStateChange("active", "active"), false);
  assert.equal(shouldSyncAfterAppStateChange("active", "background"), false);
  assert.equal(shouldSyncAfterAppStateChange("inactive", "background"), false);
});
