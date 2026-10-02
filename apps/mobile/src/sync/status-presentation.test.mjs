import assert from "node:assert/strict";
import test from "node:test";

import { syncStatusPresentation } from "./status-presentation.ts";

const ready = {
  phase: "ready",
  running: true,
  syncingNow: false,
  hasUnsentChanges: false,
  lastSyncAtMs: 1_000_000,
  errorMessage: null,
  consecutiveFailures: 0,
};

test("reports healthy/pending/retrying flags without claiming success early", () => {
  const flags = (presentation) => ({
    healthy: presentation.healthy,
    pending: presentation.pending,
    retrying: presentation.retrying,
  });

  assert.deepEqual(flags(syncStatusPresentation(ready, 1_030_000)), {
    healthy: true,
    pending: false,
    retrying: false,
  });
  assert.deepEqual(
    flags(
      syncStatusPresentation({
        ...ready,
        errorMessage: "Network unavailable",
        consecutiveFailures: 2,
      }),
    ),
    { healthy: false, pending: false, retrying: true },
  );
  assert.deepEqual(
    flags(
      syncStatusPresentation({
        ...ready,
        running: false,
        errorMessage: "Previous connection failed",
        consecutiveFailures: 1,
      }),
    ),
    { healthy: false, pending: false, retrying: false },
  );
  assert.deepEqual(
    flags(syncStatusPresentation({ ...ready, lastSyncAtMs: null })),
    { healthy: false, pending: true, retrying: false },
  );
  assert.equal(
    syncStatusPresentation({ ...ready, hasUnsentChanges: true }).pending,
    true,
  );
  assert.deepEqual(
    flags(
      syncStatusPresentation({
        ...ready,
        phase: "approval_pending",
        running: false,
        lastSyncAtMs: null,
      }),
    ),
    { healthy: false, pending: true, retrying: false },
  );
});
