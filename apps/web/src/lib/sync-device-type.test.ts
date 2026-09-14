import assert from "node:assert/strict";
import test from "node:test";

import { inferSyncDeviceType } from "./sync-device-type.ts";

test("recognizes common mobile device names", () => {
  assert.equal(inferSyncDeviceType("John's iPhone"), "mobile");
  assert.equal(inferSyncDeviceType("Pixel 10 Pro"), "mobile");
  assert.equal(inferSyncDeviceType("Galaxy Tab S11"), "mobile");
  assert.equal(inferSyncDeviceType("SM-S938B"), "mobile");
});

test("recognizes common desktop device names", () => {
  assert.equal(inferSyncDeviceType("MacBook-Pro.local"), "desktop");
  assert.equal(inferSyncDeviceType("Johns-M4-Max.local"), "desktop");
  assert.equal(inferSyncDeviceType("Mac Studio"), "desktop");
  assert.equal(inferSyncDeviceType("Windows desktop"), "desktop");
});

test("does not claim a device type for ambiguous names", () => {
  assert.equal(inferSyncDeviceType("Johndow"), "unknown");
  assert.equal(inferSyncDeviceType("Work Mac"), "unknown");
  assert.equal(inferSyncDeviceType(null), "unknown");
});
