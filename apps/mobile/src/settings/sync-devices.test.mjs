import assert from "node:assert/strict";
import { test } from "node:test";

import { parseSyncDeviceResponse } from "./sync-device-response.ts";

for (const maxDevices of [3, 5]) {
  test(`shows the server-provided ${maxDevices}-device allowance and counts reservations once`, () => {
    const result = parseSyncDeviceResponse({
      maxDevices,
      devices: [
        {
          deviceFingerprint: "desktop-1",
          deviceName: "Mac",
          deviceKind: "desktop",
        },
      ],
      pendingDevices: [
        { deviceFingerprint: "desktop-1" },
        { deviceFingerprint: "phone-2" },
      ],
    });
    assert.equal(result.maxDevices, maxDevices);
    assert.equal(result.usedDevices, 2);
    assert.equal(result.devices[0].deviceName, "Mac");
  });
}

test("rejects invalid device allowances instead of displaying a guessed limit", () => {
  for (const maxDevices of [undefined, 0, -1, 3.5, "5"]) {
    assert.throws(
      () =>
        parseSyncDeviceResponse({
          maxDevices,
          devices: [],
          pendingDevices: [],
        }),
      /Invalid device response/,
    );
  }
});
