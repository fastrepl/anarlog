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

test("shows a healthy synced state only after a successful round", () => {
  assert.deepEqual(syncStatusPresentation(ready, 1_030_000), {
    title: "Cloud sync is on",
    description: "Your notes sync end-to-end encrypted across your devices.",
    detail: "Synced just now",
    healthy: true,
    pending: false,
    retrying: false,
  });
});

test("does not label a failing runtime as healthy", () => {
  const presentation = syncStatusPresentation({
    ...ready,
    errorMessage: "Network unavailable",
    consecutiveFailures: 2,
  });

  assert.equal(presentation.title, "Cloud sync is retrying");
  assert.equal(presentation.detail, "Network unavailable");
  assert.equal(presentation.healthy, false);
  assert.equal(presentation.pending, false);
  assert.equal(presentation.retrying, true);
});

test("keeps local safety explicit when the native runtime is paused", () => {
  const presentation = syncStatusPresentation({
    ...ready,
    running: false,
    errorMessage: "Previous connection failed",
    consecutiveFailures: 1,
  });

  assert.equal(presentation.title, "Cloud sync is paused");
  assert.match(presentation.description, /safe on this device/);
  assert.equal(presentation.detail, "Previous connection failed");
  assert.equal(presentation.healthy, false);
  assert.equal(presentation.pending, false);
  assert.equal(presentation.retrying, false);
});

test("does not claim success before the first encrypted sync finishes", () => {
  const presentation = syncStatusPresentation({
    ...ready,
    lastSyncAtMs: null,
  });

  assert.equal(presentation.title, "Finishing cloud sync");
  assert.equal(presentation.detail, "Waiting for first sync");
  assert.equal(presentation.healthy, false);
  assert.equal(presentation.pending, true);
});

test("prioritizes active and pending sync detail", () => {
  assert.equal(
    syncStatusPresentation({ ...ready, syncingNow: true }).detail,
    "Syncing now…",
  );
  assert.equal(
    syncStatusPresentation({ ...ready, hasUnsentChanges: true }).detail,
    "Changes waiting to sync",
  );
  assert.equal(
    syncStatusPresentation({ ...ready, hasUnsentChanges: true }).pending,
    true,
  );
});

test("keeps device approval in the background", () => {
  const presentation = syncStatusPresentation({
    ...ready,
    phase: "approval_pending",
    running: false,
    lastSyncAtMs: null,
  });

  assert.equal(presentation.title, "Connecting this device");
  assert.match(presentation.description, /Keep working here/);
  assert.equal(presentation.healthy, false);
});
