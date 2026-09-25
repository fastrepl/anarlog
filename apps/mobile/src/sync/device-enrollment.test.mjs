import assert from "node:assert/strict";
import test from "node:test";

import {
  DeviceEnrollmentError,
  consumeDeviceEnrollment,
  requestDeviceEnrollment,
  shareDeviceEnrollments,
} from "./device-enrollment.ts";

const input = {
  apiUrl: "https://api.anarlog.test",
  accessToken: "access-token",
  publicKey: "A".repeat(43),
  device: { fingerprint: "device-1234", name: "John's iPhone" },
};

test("registers the mobile device for managed key enrollment", async () => {
  let request;
  const enrollment = await requestDeviceEnrollment({
    ...input,
    fetcher: async (url, init) => {
      request = { url: String(url), init };
      return new Response(
        JSON.stringify({
          requestId: "72e4b975-e6cb-4c00-a8a0-e61c55272377",
          status: "pending",
          package: null,
        }),
      );
    },
  });

  assert.equal(
    request.url,
    "https://api.anarlog.test/sync/e2ee/device-enrollments",
  );
  assert.equal(request.init.headers["X-Device-Fingerprint"], "device-1234");
  assert.equal(request.init.headers["X-Anarlog-Device-Name"], "John's iPhone");
  assert.equal(enrollment.status, "pending");
});

test("maps a missing account key to first-device bootstrap", async () => {
  const error = await requestDeviceEnrollment({
    ...input,
    fetcher: async () =>
      new Response(
        JSON.stringify({
          error: { code: "e2ee_enrollment_requires_existing_key" },
        }),
        { status: 409 },
      ),
  }).catch((value) => value);

  assert.ok(error instanceof DeviceEnrollmentError);
  assert.equal(error.code, "first_device");
});

test("consumes an approved enrollment package", async () => {
  let request;
  await consumeDeviceEnrollment({
    apiUrl: input.apiUrl,
    accessToken: input.accessToken,
    requestId: "72e4b975-e6cb-4c00-a8a0-e61c55272377",
    publicKey: input.publicKey,
    fingerprint: input.device.fingerprint,
    fetcher: async (url, init) => {
      request = { url: String(url), init };
      return new Response(null, { status: 204 });
    },
  });

  assert.match(request.url, /device-enrollments\/72e4b975.*\/consume$/);
  assert.deepEqual(JSON.parse(request.init.body), {
    publicKey: input.publicKey,
  });
});

test("shares only valid pending requests, tolerating another device sealing first", async () => {
  const device = {
    requestId: "72e4b975-e6cb-4c00-a8a0-e61c55272377",
    status: "pending",
    publicKey: "A".repeat(43),
    expiresAt: "2099-01-01",
  };
  const calls = [];
  const packages = [];
  await shareDeviceEnrollments({
    ...input,
    signal: new AbortController().signal,
    seal: async (...args) => {
      packages.push(args);
      return { ciphertext: "encrypted" };
    },
    fetcher: async (url, init) => {
      calls.push([String(url), init]);
      if (calls.length === 1)
        return new Response(
          JSON.stringify({
            pendingDevices: [
              { ...device, status: "sealed" },
              { ...device, expiresAt: "invalid" },
              device,
              { ...device, requestId: "../../bad" },
            ],
          }),
        );
      return new Response(null, { status: 409 });
    },
  });
  assert.equal(calls.length, 2);
  assert.deepEqual(packages, [[device.requestId, device.publicKey]]);
  assert.equal(calls[1][1].headers.Authorization, "Bearer access-token");
  assert.deepEqual(JSON.parse(calls[1][1].body), { ciphertext: "encrypted" });
});
test("does not publish after cancellation while sealing", async () => {
  const controller = new AbortController();
  let requests = 0;
  await shareDeviceEnrollments({
    ...input,
    signal: controller.signal,
    seal: async () => {
      controller.abort();
      return { ciphertext: "encrypted" };
    },
    fetcher: async () => {
      requests++;
      return new Response(
        JSON.stringify({
          pendingDevices: [
            {
              requestId: "72e4b975-e6cb-4c00-a8a0-e61c55272377",
              status: "pending",
              publicKey: "A".repeat(43),
              expiresAt: "2099-01-01",
            },
          ],
        }),
      );
    },
  });
  assert.equal(requests, 1);
});
