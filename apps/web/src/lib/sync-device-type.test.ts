import assert from "node:assert/strict";
import test from "node:test";

import { inferSyncDeviceType } from "./sync-device-type.ts";

test("infers the sync device type from common device names", () => {
  const cases = [
    ["John's iPhone", "mobile"],
    ["Pixel 10 Pro", "mobile"],
    ["Galaxy Tab S11", "mobile"],
    ["SM-S938B", "mobile"],
    ["MacBook-Pro.local", "desktop"],
    ["Johns-M4-Max.local", "desktop"],
    ["Mac Studio", "desktop"],
    ["Windows desktop", "desktop"],
    ["Johndow", "unknown"],
    ["Work Mac", "unknown"],
    [null, "unknown"],
  ] as const;

  for (const [name, expected] of cases) {
    assert.equal(inferSyncDeviceType(name), expected, name ?? "null");
  }
});
