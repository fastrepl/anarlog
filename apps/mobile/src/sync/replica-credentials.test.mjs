import assert from "node:assert/strict";
import test from "node:test";

import {
  ReplicaCredentialError,
  requestReplicaCredentials,
} from "./replica-credentials.ts";

const identity = {
  keyId: "ABCDEFGHIJKLMNOPQRSTUV",
  memberPublicKey: "A".repeat(43),
};

const credentials = {
  transport: "replica",
  encryptionVersion: 2,
  encryptionKeyId: identity.keyId,
  expiresAt: "2026-08-18T00:00:00Z",
  workspaceId: "user-123",
  accountUserId: "user-123",
};

test("requests validated replica credentials with the E2EE identity", async () => {
  let request;
  const result = await requestReplicaCredentials({
    apiUrl: "https://api.anarlog.test/base",
    accessToken: "access-token",
    accountUserId: "user-123",
    identity,
    device: { fingerprint: "device-1234", name: "John's iPhone" },
    fetcher: async (input, init) => {
      request = { input, init };
      return new Response(JSON.stringify(credentials), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    },
  });

  assert.deepEqual(result, credentials);
  assert.equal(
    request.input.toString(),
    "https://api.anarlog.test/sync/replica/credentials",
  );
  assert.equal(request.init.method, "POST");
  const headers = new Headers(request.init.headers);
  assert.equal(headers.get("authorization"), "Bearer access-token");
  assert.equal(headers.get("x-anarlog-e2ee-key-id"), identity.keyId);
  assert.equal(
    headers.get("x-anarlog-e2ee-member-public-key"),
    identity.memberPublicKey,
  );
  assert.equal(headers.get("x-device-fingerprint"), "device-1234");
  assert.equal(headers.get("x-anarlog-device-name"), "John's iPhone");
});

test("rejects credentials that do not match the signed-in identity", async () => {
  await assert.rejects(
    requestReplicaCredentials({
      apiUrl: "https://api.anarlog.test",
      accessToken: "access-token",
      accountUserId: "user-123",
      identity,
      fetcher: async () =>
        new Response(
          JSON.stringify({ ...credentials, accountUserId: "user-456" }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
    }),
    (error) =>
      error instanceof ReplicaCredentialError &&
      error.code === "invalid_response",
  );
});

test("surfaces the sync device limit distinctly", async () => {
  await assert.rejects(
    requestReplicaCredentials({
      apiUrl: "https://api.anarlog.test",
      accessToken: "access-token",
      accountUserId: "user-123",
      identity,
      fetcher: async () =>
        new Response(
          JSON.stringify({
            error: { code: "sync_device_limit_reached" },
          }),
          { status: 403, headers: { "Content-Type": "application/json" } },
        ),
    }),
    (error) =>
      error instanceof ReplicaCredentialError && error.code === "device_limit",
  );
});

test("rejects a malformed local identity before making a request", async () => {
  let requested = false;
  await assert.rejects(
    requestReplicaCredentials({
      apiUrl: "https://api.anarlog.test",
      accessToken: "access-token",
      accountUserId: "user-123",
      identity: { keyId: "invalid", memberPublicKey: "invalid" },
      fetcher: async () => {
        requested = true;
        return new Response();
      },
    }),
    (error) =>
      error instanceof ReplicaCredentialError &&
      error.code === "invalid_response",
  );
  assert.equal(requested, false);
});

test("bounds an unavailable credential request", async () => {
  await assert.rejects(
    requestReplicaCredentials({
      apiUrl: "https://api.anarlog.test",
      accessToken: "access-token",
      accountUserId: "user-123",
      identity,
      timeoutMs: 0,
      fetcher: async (_input, init) =>
        await new Promise((_resolve, reject) => {
          init.signal.addEventListener("abort", () => {
            reject(new Error("aborted"));
          });
        }),
    }),
    (error) =>
      error instanceof ReplicaCredentialError && error.code === "unavailable",
  );
});
