import assert from "node:assert/strict";
import test from "node:test";

import {
  DeviceEnrollmentError,
  consumeDeviceEnrollment,
  requestDeviceEnrollment,
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
