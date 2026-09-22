import assert from "node:assert/strict";
import { test } from "node:test";

import {
  consumeDrivePickerHandoff,
  drivePickerCallback,
  prepareDrivePickerHandoff,
} from "./drive-picker.ts";
const requestId = "12345678-1234-4234-9234-123456789abc";

test("desktop callback rejects unknown schemes and carries encoded results", () => {
  assert.throws(() => drivePickerCallback("https", requestId, "result"));
  assert.throws(() => drivePickerCallback("anarlog", "invalid", "result"));
  assert.equal(
    new URL(
      drivePickerCallback("anarlog-dev", requestId, "result"),
    ).searchParams.get("return_to"),
    `drive-picker:${requestId}:result`,
  );
});

test("direct OAuth handoff binds the same tab and clears authorization codes before telemetry", () => {
  const storage = new Map<string, string>();
  let cleared = "";
  const location = {
    pathname: "/app/google-drive-picker",
    search: "?flow=desktop&scheme=anarlog-dev&callback_port=1425",
    hash: "",
  };
  const auth = new URL(
    "https://accounts.google.com/o/oauth2/v2/auth?state=signed-state",
  );
  location.hash = new URLSearchParams({
    authorization_url: auth.toString(),
    request_id: requestId,
  }).toString();
  location.hash = "#" + location.hash;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      location,
      history: {
        state: {},
        replaceState: (_a: unknown, _b: unknown, url: string) => {
          cleared = url;
        },
      },
      sessionStorage: {
        setItem: (k: string, v: string) => storage.set(k, v),
        getItem: (k: string) => storage.get(k) ?? null,
        removeItem: (k: string) => storage.delete(k),
      },
    },
  });
  try {
    prepareDrivePickerHandoff();
    assert.equal(
      consumeDrivePickerHandoff()?.authorizationUrl,
      auth.toString(),
    );
    assert.equal(cleared, location.pathname);
    assert.equal(consumeDrivePickerHandoff(), null);
    location.hash = "";
    location.search = "?state=wrong&code=secret&picked_file_ids=folder";
    prepareDrivePickerHandoff();
    assert.equal(consumeDrivePickerHandoff(), null);
    location.search = "?state=signed-state&code=secret&picked_file_ids=folder";
    prepareDrivePickerHandoff();
    const result = consumeDrivePickerHandoff()!;
    assert.equal(result.port, 1425);
    const returned = new URL(result.callback!).searchParams
      .get("return_to")!
      .split(":")[2];
    assert.deepEqual(JSON.parse(decodeURIComponent(returned)), {
      state: "signed-state",
      code: "secret",
      folder_id: "folder",
    });
    assert.equal(cleared, location.pathname);
    prepareDrivePickerHandoff();
    assert.equal(consumeDrivePickerHandoff(), null);
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});
