import assert from "node:assert/strict";
import test from "node:test";

import { MobileSyncController } from "./controller.ts";

const session = {
  apiUrl: "https://api.anarlog.test",
  accessToken: "access-token",
  accountUserId: "user-123",
};

const status = {
  configured: true,
  running: true,
  has_unsent_changes: false,
  last_sync_at_ms: 1234,
  last_error: null,
  consecutive_failures: 0,
};

async function waitFor(predicate) {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (predicate()) return;
    await new Promise((resolve) => setImmediate(resolve));
  }
  assert.fail("condition was not reached");
}

function dependencies(overrides = {}) {
  return {
    readRecoveryKey: async () => "recovery-key",
    saveRecoveryKey: async () => {},
    deleteRecoveryKey: async () => {},
    generateRecoveryKey: async () => "generated-recovery-key",
    inspectRecoveryKey: async () => ({
      keyId: "ABCDEFGHIJKLMNOPQRSTUV",
      memberPublicKey: "A".repeat(43),
    }),
    claimIdentity: async () => {},
    getDevice: async () => ({ fingerprint: "device-1234" }),
    bootstrap: async () => "configured",
    stop: async () => {},
    syncNow: async () => {},
    getStatus: async () => status,
    reportError: () => {},
    ...overrides,
  };
}

test("requires explicit recovery-key setup when this account has no key", async () => {
  const controller = new MobileSyncController(
    dependencies({ readRecoveryKey: async () => null }),
    0,
    0,
  );
  controller.activate(session);

  await waitFor(() => controller.getSnapshot().phase === "setup_required");
  assert.equal(controller.getSnapshot().running, false);
});

test("boots the native replica with the stored account key", async () => {
  let bootstrapArguments;
  const controller = new MobileSyncController(
    dependencies({
      bootstrap: async (...args) => {
        bootstrapArguments = args;
        return "configured";
      },
    }),
    0,
    0,
  );
  controller.activate(session);

  await waitFor(() => controller.getSnapshot().phase === "ready");
  assert.deepEqual(bootstrapArguments, [
    session,
    "recovery-key",
    { fingerprint: "device-1234" },
  ]);
  assert.deepEqual(controller.getSnapshot(), {
    phase: "ready",
    running: true,
    syncingNow: false,
    hasUnsentChanges: false,
    lastSyncAtMs: 1234,
    errorMessage: null,
    consecutiveFailures: 0,
  });
});

test("ignores a stale activation before booting the next account", async () => {
  let resolveFirstRead;
  const readRecoveryKey = (accountUserId) => {
    if (accountUserId === "user-123") {
      return new Promise((resolve) => {
        resolveFirstRead = resolve;
      });
    }
    return Promise.resolve("second-key");
  };
  const bootstrappedAccounts = [];
  const controller = new MobileSyncController(
    dependencies({
      readRecoveryKey,
      bootstrap: async (activeSession) => {
        bootstrappedAccounts.push(activeSession.accountUserId);
        return "configured";
      },
    }),
    0,
    0,
  );
  controller.activate(session);
  await waitFor(() => resolveFirstRead !== undefined);
  controller.activate({ ...session, accountUserId: "user-456" });
  resolveFirstRead("first-key");

  await waitFor(() => controller.getSnapshot().phase === "ready");
  assert.deepEqual(bootstrappedAccounts, ["user-456"]);
});

test("stores and returns a generated key before replica startup completes", async () => {
  let saved;
  let resolveBootstrap;
  const controller = new MobileSyncController(
    dependencies({
      readRecoveryKey: async () => saved ?? null,
      saveRecoveryKey: async (_accountUserId, recoveryKey) => {
        saved = recoveryKey;
      },
      bootstrap: async () =>
        await new Promise((resolve) => {
          resolveBootstrap = resolve;
        }),
    }),
    0,
    0,
  );
  controller.activate(session);
  await waitFor(() => controller.getSnapshot().phase === "setup_required");

  const recoveryKey = await controller.createRecoveryKey();
  assert.equal(recoveryKey, "generated-recovery-key");
  assert.equal(saved, recoveryKey);
  await waitFor(() => resolveBootstrap !== undefined);
  resolveBootstrap("configured");
  await waitFor(() => controller.getSnapshot().phase === "ready");
});

test("rolls back secure storage when the server rejects a new identity", async () => {
  let saved = null;
  let deleted = false;
  const controller = new MobileSyncController(
    dependencies({
      readRecoveryKey: async () => saved,
      saveRecoveryKey: async (_accountUserId, recoveryKey) => {
        saved = recoveryKey;
      },
      deleteRecoveryKey: async () => {
        deleted = true;
        saved = null;
      },
      claimIdentity: async () => {
        throw new Error("identity mismatch");
      },
    }),
    0,
    0,
  );
  controller.activate(session);
  await waitFor(() => controller.getSnapshot().phase === "setup_required");

  await assert.rejects(controller.createRecoveryKey(), /identity mismatch/);
  assert.equal(deleted, true);
  assert.equal(saved, null);
});

test("uses the refreshed session when setup finishes", async () => {
  let resolveGeneratedKey;
  let saved = null;
  let claimedAccessToken;
  let bootstrappedAccessToken;
  const controller = new MobileSyncController(
    dependencies({
      readRecoveryKey: async () => saved,
      saveRecoveryKey: async (_accountUserId, recoveryKey) => {
        saved = recoveryKey;
      },
      generateRecoveryKey: async () =>
        await new Promise((resolve) => {
          resolveGeneratedKey = resolve;
        }),
      claimIdentity: async (activeSession) => {
        claimedAccessToken = activeSession.accessToken;
      },
      bootstrap: async (activeSession) => {
        bootstrappedAccessToken = activeSession.accessToken;
        return "configured";
      },
    }),
    0,
    0,
  );
  controller.activate(session);
  await waitFor(() => controller.getSnapshot().phase === "setup_required");
  const setup = controller.createRecoveryKey();
  await waitFor(() => resolveGeneratedKey !== undefined);

  controller.activate({ ...session, accessToken: "refreshed-token" });
  resolveGeneratedKey("generated-recovery-key");
  await setup;
  await waitFor(() => controller.getSnapshot().phase === "ready");

  assert.equal(claimedAccessToken, "refreshed-token");
  assert.equal(bootstrappedAccessToken, "refreshed-token");
});

test("stops native sync when the account lifecycle ends", async () => {
  let stopCount = 0;
  const controller = new MobileSyncController(
    dependencies({
      stop: async () => {
        stopCount += 1;
      },
    }),
    0,
    0,
  );
  controller.activate(session);
  await waitFor(() => controller.getSnapshot().phase === "ready");

  controller.suspend();
  await waitFor(() => stopCount === 2);
  assert.equal(controller.getSnapshot().phase, "inactive");
});
