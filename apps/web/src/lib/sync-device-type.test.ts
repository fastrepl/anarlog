import assert from "node:assert/strict";
import test from "node:test";

import { inferSyncDeviceType } from "./sync-device-type.ts";

test("recognizes common mobile device names", () => {
  assert.equal(inferSyncDeviceType("John's iPhone"), "mobile");
  assert.equal(inferSyncDeviceType("Pixel 10 Pro"), "mobile");
  assert.equal(inferSyncDeviceType("Galaxy Tab S11"), "mobile");
});

test("uses a desktop icon for hostnames and missing device names", () => {
  assert.equal(inferSyncDeviceType("MacBook-Pro.local"), "desktop");
  assert.equal(inferSyncDeviceType("Johndow"), "desktop");
  assert.equal(inferSyncDeviceType(null), "desktop");
});
