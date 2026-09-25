import { afterEach, expect, test, vi } from "vitest";

import {
  createE2eeIdentity,
  importE2eeIdentity,
  inspectE2eeRecoveryKey,
} from "@anlg/plugin-db";

import { establishAutomaticSyncIdentity } from "./automatic-sync-identity";
vi.mock("~/env", () => ({ env: { VITE_API_URL: "https://api.test" } }));
afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});
test("claims a durable first-device key before importing it", async () => {
  vi.mocked(createE2eeIdentity).mockResolvedValue("durable-key");
  vi.mocked(inspectE2eeRecoveryKey).mockResolvedValue({ keyId: "key-id" });
  const fetchMock = vi
    .fn()
    .mockResolvedValue(new Response(JSON.stringify({ keyId: "key-id" })));
  vi.stubGlobal("fetch", fetchMock);
  expect(
    await establishAutomaticSyncIdentity("account", "token", () => true),
  ).toBe(true);
  expect(importE2eeIdentity).toHaveBeenCalledWith("account", "durable-key");
  expect(fetchMock).toHaveBeenCalledWith(
    new URL("https://api.test/sync/e2ee/identity"),
    expect.objectContaining({ body: JSON.stringify({ keyId: "key-id" }) }),
  );
});
test("uses device enrollment when another device already claimed the account", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(new Response(null, { status: 409 })),
  );
  expect(
    await establishAutomaticSyncIdentity("account", "token", () => true),
  ).toBe(false);
  expect(importE2eeIdentity).not.toHaveBeenCalled();
});
test("does not claim or import after the session changes", async () => {
  const fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
  expect(
    await establishAutomaticSyncIdentity("account", "token", () => false),
  ).toBe(false);
  expect(fetchMock).not.toHaveBeenCalled();
  expect(importE2eeIdentity).not.toHaveBeenCalled();
});

test("never imports a mismatched server key identity", async () => {
  vi.stubGlobal(
    "fetch",
    vi
      .fn()
      .mockResolvedValue(new Response(JSON.stringify({ keyId: "other-key" }))),
  );
  await expect(
    establishAutomaticSyncIdentity("account", "token", () => true),
  ).rejects.toThrow("Unexpected sync identity");
  expect(importE2eeIdentity).not.toHaveBeenCalled();
});

test("does not import if the account changes while claiming", async () => {
  let current = true;
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation(async () => {
      current = false;
      return new Response(JSON.stringify({ keyId: "key-id" }));
    }),
  );
  expect(
    await establishAutomaticSyncIdentity("account", "token", () => current),
  ).toBe(false);
  expect(importE2eeIdentity).not.toHaveBeenCalled();
});
