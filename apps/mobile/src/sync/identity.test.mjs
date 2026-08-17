import assert from "node:assert/strict";
import test from "node:test";

import { claimReplicaIdentity } from "./identity.ts";
import { ReplicaCredentialError } from "./replica-credentials.ts";

const keyId = "ABCDEFGHIJKLMNOPQRSTUV";

test("claims the account identity with the inspected key id", async () => {
  let request;
  await claimReplicaIdentity({
    apiUrl: "https://api.anarlog.test/base",
    accessToken: "access-token",
    keyId,
    fetcher: async (input, init) => {
      request = { input, init };
      return Response.json({ keyId });
    },
  });

  assert.equal(
    request.input.toString(),
    "https://api.anarlog.test/sync/e2ee/identity",
  );
  assert.equal(request.init.method, "PUT");
  assert.equal(
    new Headers(request.init.headers).get("authorization"),
    "Bearer access-token",
  );
  assert.deepEqual(JSON.parse(request.init.body), { keyId });
});

test("does not replace an existing account identity", async () => {
  await assert.rejects(
    claimReplicaIdentity({
      apiUrl: "https://api.anarlog.test",
      accessToken: "access-token",
      keyId,
      fetcher: async () => new Response(null, { status: 409 }),
    }),
    (error) =>
      error instanceof ReplicaCredentialError &&
      error.code === "identity_mismatch",
  );
});

test("rejects a response for another identity", async () => {
  await assert.rejects(
    claimReplicaIdentity({
      apiUrl: "https://api.anarlog.test",
      accessToken: "access-token",
      keyId,
      fetcher: async () => Response.json({ keyId: "B".repeat(22) }),
    }),
    (error) =>
      error instanceof ReplicaCredentialError &&
      error.code === "invalid_response",
  );
});
