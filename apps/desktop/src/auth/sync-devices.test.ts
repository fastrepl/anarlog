import { afterEach, expect, test, vi } from "vitest";

import {
  SyncDeviceRequestError,
  consumeDeviceEnrollment,
  registerDeviceEnrollment,
  removeSyncDevice,
  renameSyncDevice,
  requestSyncDevices,
  sealDeviceEnrollment,
} from "./sync-devices";

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.test" },
}));

afterEach(() => {
  vi.unstubAllGlobals();
});

test("validates active and pending device responses", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(() =>
      Promise.resolve(
        Response.json({
          devices: [
            {
              deviceFingerprint: "fingerprint-active",
              deviceName: "Active Mac",
              deviceKind: "desktop",
              createdAt: "2026-08-19T00:00:00Z",
              lastSeenAt: "2026-08-20T00:00:00Z",
            },
          ],
          pendingDevices: [
            {
              requestId: "11111111-1111-4111-8111-111111111111",
              deviceFingerprint: "fingerprint-pending",
              deviceName: "Pending Mac",
              deviceKind: "mobile",
              publicKey: "A".repeat(43),
              createdAt: "2026-08-20T00:00:00Z",
              expiresAt: "2026-08-21T00:00:00Z",
              status: "pending",
            },
          ],
          maxDevices: 5,
        }),
      ),
    ),
  );

  await expect(requestSyncDevices("access-token")).resolves.toMatchObject({
    maxDevices: 5,
    devices: [
      { deviceFingerprint: "fingerprint-active", deviceKind: "desktop" },
    ],
    pendingDevices: [{ status: "pending", deviceKind: "mobile" }],
  });
});

test("registers a device with replacement metadata", async () => {
  const fetchMock = vi.fn<
    (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
  >(() =>
    Promise.resolve(
      Response.json({
        requestId: "11111111-1111-4111-8111-111111111111",
        expiresAt: "2026-08-21T00:00:00Z",
        status: "pending",
        package: null,
      }),
    ),
  );
  vi.stubGlobal("fetch", fetchMock);

  await registerDeviceEnrollment({
    accessToken: "access-token",
    publicKey: "A".repeat(43),
    fingerprint: "fingerprint-current",
    deviceName: "Current Mac",
    replaceFingerprint: "fingerprint-old",
  });

  const [url, request] = fetchMock.mock.calls[0]!;
  expect(url.toString()).toBe("https://api.test/sync/e2ee/device-enrollments");
  expect(request).toMatchObject({
    method: "POST",
    headers: {
      Authorization: "Bearer access-token",
      "Content-Type": "application/json",
      "x-anarlog-device-name": "Current Mac",
      "x-device-fingerprint": "fingerprint-current",
    },
  });
  expect(JSON.parse(String(request?.body))).toEqual({
    publicKey: "A".repeat(43),
    replaceFingerprint: "fingerprint-old",
  });
});

test("surfaces structured enrollment errors", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(() =>
      Promise.resolve(
        Response.json(
          {
            error: {
              code: "sync_device_limit_reached",
              message: "CloudSync device limit reached",
            },
          },
          { status: 403 },
        ),
      ),
    ),
  );

  const error = await registerDeviceEnrollment({
    accessToken: "access-token",
    publicKey: "A".repeat(43),
    fingerprint: "fingerprint-current",
    deviceName: null,
  }).catch((reason: unknown) => reason);

  expect(error).toBeInstanceOf(SyncDeviceRequestError);
  expect(error).toMatchObject({
    status: 403,
    code: "sync_device_limit_reached",
  });
});

test("posts approval, acknowledgement, removal, and rename requests", async () => {
  const fetchMock = vi.fn<
    (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
  >(() => Promise.resolve(new Response(null, { status: 204 })));
  vi.stubGlobal("fetch", fetchMock);
  const packageValue = {
    ephemeralPublicKey: "E".repeat(43),
    nonce: "N".repeat(32),
    ciphertext: "C".repeat(100),
  };

  await sealDeviceEnrollment({
    accessToken: "access-token",
    requestId: "11111111-1111-4111-8111-111111111111",
    packageValue,
  });
  await consumeDeviceEnrollment({
    accessToken: "access-token",
    requestId: "11111111-1111-4111-8111-111111111111",
    publicKey: "A".repeat(43),
    fingerprint: "fingerprint-current",
  });
  await removeSyncDevice("access-token", "fingerprint-old");
  await renameSyncDevice("access-token", "fingerprint-current", "Desk Mac");

  expect(fetchMock.mock.calls.map(([url]) => url.toString())).toEqual([
    "https://api.test/sync/e2ee/device-enrollments/11111111-1111-4111-8111-111111111111/seal",
    "https://api.test/sync/e2ee/device-enrollments/11111111-1111-4111-8111-111111111111/consume",
    "https://api.test/sync/devices/fingerprint-old",
    "https://api.test/sync/devices/fingerprint-current",
  ]);
  expect(fetchMock.mock.calls[3]?.[1]).toMatchObject({
    method: "PATCH",
    headers: {
      Authorization: "Bearer access-token",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ deviceName: "Desk Mac" }),
  });
});
