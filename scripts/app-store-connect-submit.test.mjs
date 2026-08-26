import assert from "node:assert/strict";
import { createVerify, generateKeyPairSync } from "node:crypto";
import test from "node:test";

import {
  AppStoreConnectError,
  createAppStoreConnectClient,
  createAppStoreConnectToken,
  normalizePrivateKey,
  publishMacApp,
} from "./app-store-connect-submit.mjs";

function resource(type, id, attributes = {}, relationships) {
  return { attributes, id, relationships, type };
}

test("creates an App Store Connect ES256 token", () => {
  const { privateKey, publicKey } = generateKeyPairSync("ec", {
    namedCurve: "P-256",
  });
  const now = 1_800_000_000_000;
  const token = createAppStoreConnectToken({
    issuerId: "issuer-id",
    keyId: "key-id",
    now: () => now,
    privateKey,
  });
  const [encodedHeader, encodedPayload, encodedSignature] = token.split(".");
  const header = JSON.parse(Buffer.from(encodedHeader, "base64url"));
  const payload = JSON.parse(Buffer.from(encodedPayload, "base64url"));

  assert.deepEqual(header, { alg: "ES256", kid: "key-id", typ: "JWT" });
  assert.deepEqual(payload, {
    aud: "appstoreconnect-v1",
    exp: Math.floor(now / 1000) + 19 * 60,
    iat: Math.floor(now / 1000),
    iss: "issuer-id",
  });
  const verifier = createVerify("SHA256");
  verifier.update(`${encodedHeader}.${encodedPayload}`);
  verifier.end();
  assert.equal(
    verifier.verify(
      { dsaEncoding: "ieee-p1363", key: publicKey },
      Buffer.from(encodedSignature, "base64url"),
    ),
    true,
  );
});

test("normalizes a JSON-escaped App Store Connect private key", () => {
  const { privateKey } = generateKeyPairSync("ec", {
    namedCurve: "P-256",
  });
  const pem = privateKey.export({ format: "pem", type: "pkcs8" });
  const normalized = normalizePrivateKey(JSON.stringify(pem));

  assert.equal(normalized, pem);
  assert.doesNotThrow(() =>
    createAppStoreConnectToken({
      issuerId: "issuer-id",
      keyId: "key-id",
      privateKey: normalized,
    }),
  );
});

test("surfaces App Store Connect API errors", async () => {
  const { privateKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
  const client = createAppStoreConnectClient({
    fetchImpl: async () =>
      new Response(
        JSON.stringify({
          errors: [
            {
              code: "ENTITY_ERROR",
              detail: "The build is not ready",
              status: "409",
              title: "Conflict",
            },
          ],
        }),
        { status: 409 },
      ),
    issuerId: "issuer-id",
    keyId: "key-id",
    privateKey,
  });

  await assert.rejects(
    client.request("GET", "/v1/apps"),
    (error) =>
      error instanceof AppStoreConnectError &&
      error.status === 409 &&
      /The build is not ready/.test(error.message),
  );
});

test("reuses the editable first version and submits a processed build", async () => {
  const requests = [];
  let uploaded = false;
  const client = {
    async request(method, pathname, options = {}) {
      requests.push({ method, pathname, ...options });
      if (pathname === "/v1/apps") {
        return { data: [resource("apps", "app-id")] };
      }
      if (pathname === "/v1/apps/app-id/appStoreVersions") {
        return {
          data: [
            resource("appStoreVersions", "placeholder-id", {
              appStoreState: "PREPARE_FOR_SUBMISSION",
              platform: "MAC_OS",
              releaseType: "MANUAL",
              versionString: "1.0",
            }),
          ],
        };
      }
      if (
        method === "PATCH" &&
        pathname === "/v1/appStoreVersions/placeholder-id"
      ) {
        return {
          data: resource("appStoreVersions", "placeholder-id", {
            appStoreState: "PREPARE_FOR_SUBMISSION",
            platform: "MAC_OS",
            releaseType: "AFTER_APPROVAL",
            versionString: "1.4.13",
          }),
        };
      }
      if (pathname === "/v1/builds") {
        return uploaded
          ? {
              data: [
                resource("builds", "build-id", {
                  processingState: "VALID",
                  uploadedDate: "2026-08-25T00:00:00Z",
                  version: "2608.26.1",
                }),
              ],
            }
          : { data: [] };
      }
      if (
        pathname === "/v1/appStoreVersions/placeholder-id/relationships/build"
      ) {
        return null;
      }
      if (pathname === "/v1/apps/app-id/reviewSubmissions") {
        return { data: [] };
      }
      if (method === "POST" && pathname === "/v1/reviewSubmissions") {
        return { data: resource("reviewSubmissions", "submission-id") };
      }
      if (pathname === "/v1/reviewSubmissions/submission-id/items") {
        return { data: [] };
      }
      if (method === "POST" && pathname === "/v1/reviewSubmissionItems") {
        return { data: resource("reviewSubmissionItems", "item-id") };
      }
      if (
        method === "PATCH" &&
        pathname === "/v1/reviewSubmissions/submission-id"
      ) {
        return { data: resource("reviewSubmissions", "submission-id") };
      }
      throw new Error(`Unexpected request ${method} ${pathname}`);
    },
  };

  const result = await publishMacApp({
    buildVersion: "2608.26.1",
    bundleId: "com.hyprnote.desktop",
    client,
    packagePath: "/tmp/Anarlog.pkg",
    pollIntervalMs: 0,
    sleep: async () => {},
    upload: async () => {
      uploaded = true;
    },
    version: "1.4.13",
  });

  assert.deepEqual(result, {
    alreadySubmitted: false,
    appStoreVersionId: "placeholder-id",
    buildId: "build-id",
    reviewSubmissionId: "submission-id",
  });
  assert.equal(uploaded, true);
  assert.deepEqual(
    requests.find(
      (request) =>
        request.method === "PATCH" &&
        request.pathname === "/v1/appStoreVersions/placeholder-id",
    ).body.data.attributes,
    { releaseType: "AFTER_APPROVAL", versionString: "1.4.13" },
  );
  assert.equal(
    requests.find((request) => request.pathname === "/v1/builds").query[
      "filter[version]"
    ],
    "2608.26.1",
  );
  assert.deepEqual(requests.at(-1).body.data.attributes, { submitted: true });
});

test("reuses an existing processing build instead of uploading it again", async () => {
  let buildRequestCount = 0;
  let uploadCount = 0;
  const client = {
    async request(method, pathname) {
      if (pathname === "/v1/apps") {
        return { data: [resource("apps", "app-id")] };
      }
      if (pathname === "/v1/apps/app-id/appStoreVersions") {
        return {
          data: [
            resource("appStoreVersions", "version-id", {
              appStoreState: "PREPARE_FOR_SUBMISSION",
              platform: "MAC_OS",
              releaseType: "AFTER_APPROVAL",
              versionString: "1.4.13",
            }),
          ],
        };
      }
      if (pathname === "/v1/builds") {
        buildRequestCount += 1;
        return {
          data: [
            resource("builds", "build-id", {
              processingState: buildRequestCount === 1 ? "PROCESSING" : "VALID",
              version: "1.4.13",
            }),
          ],
        };
      }
      if (pathname === "/v1/appStoreVersions/version-id/relationships/build") {
        return null;
      }
      if (pathname === "/v1/apps/app-id/reviewSubmissions") {
        return {
          data: [resource("reviewSubmissions", "submission-id")],
        };
      }
      if (pathname === "/v1/reviewSubmissions/submission-id/items") {
        return {
          data: [
            resource(
              "reviewSubmissionItems",
              "item-id",
              {},
              {
                appStoreVersion: {
                  data: { id: "version-id", type: "appStoreVersions" },
                },
              },
            ),
          ],
        };
      }
      if (
        method === "PATCH" &&
        pathname === "/v1/reviewSubmissions/submission-id"
      ) {
        return { data: resource("reviewSubmissions", "submission-id") };
      }
      throw new Error(`Unexpected request ${method} ${pathname}`);
    },
  };

  const result = await publishMacApp({
    buildVersion: "1.4.13",
    bundleId: "com.hyprnote.desktop",
    client,
    packagePath: "/tmp/Anarlog.pkg",
    pollIntervalMs: 0,
    sleep: async () => {},
    upload: async () => {
      uploadCount += 1;
    },
    version: "1.4.13",
  });

  assert.equal(result.buildId, "build-id");
  assert.equal(uploadCount, 0);
  assert.equal(buildRequestCount, 2);
});

test("treats an already submitted version as an idempotent success", async () => {
  const requests = [];
  const client = {
    async request(method, pathname) {
      requests.push({ method, pathname });
      if (pathname === "/v1/apps") {
        return { data: [resource("apps", "app-id")] };
      }
      if (pathname === "/v1/apps/app-id/appStoreVersions") {
        return {
          data: [
            resource("appStoreVersions", "version-id", {
              appStoreState: "WAITING_FOR_REVIEW",
              platform: "MAC_OS",
              releaseType: "AFTER_APPROVAL",
              versionString: "1.4.13",
            }),
          ],
        };
      }
      throw new Error(`Unexpected request ${method} ${pathname}`);
    },
  };

  const result = await publishMacApp({
    buildVersion: "1.4.13",
    bundleId: "com.hyprnote.desktop",
    client,
    packagePath: "/tmp/Anarlog.pkg",
    upload: async () => assert.fail("upload should not run"),
    version: "1.4.13",
  });

  assert.deepEqual(result, {
    alreadySubmitted: true,
    appStoreVersionId: "version-id",
  });
  assert.equal(requests.length, 2);
});

test("refuses to choose between multiple editable placeholder versions", async () => {
  const client = {
    async request(_method, pathname) {
      if (pathname === "/v1/apps") {
        return { data: [resource("apps", "app-id")] };
      }
      if (pathname === "/v1/apps/app-id/appStoreVersions") {
        return {
          data: ["one", "two"].map((id) =>
            resource("appStoreVersions", id, {
              appStoreState: "PREPARE_FOR_SUBMISSION",
              platform: "MAC_OS",
              releaseType: "MANUAL",
              versionString: id,
            }),
          ),
        };
      }
      throw new Error(`Unexpected request ${pathname}`);
    },
  };

  await assert.rejects(
    publishMacApp({
      buildVersion: "1.4.13",
      bundleId: "com.hyprnote.desktop",
      client,
      packagePath: "/tmp/Anarlog.pkg",
      upload: async () => {},
      version: "1.4.13",
    }),
    /cannot choose one safely/,
  );
});
