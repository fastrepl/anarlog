import assert from "node:assert/strict";
import test from "node:test";

import { backgroundSyncFailed, backgroundSyncWork } from "./background-work.ts";

const ready = {
  phase: "ready",
  running: true,
  syncingNow: false,
  hasUnsentChanges: false,
  consecutiveFailures: 0,
  errorMessage: null,
};

test("keeps sync alive while local changes or uploads are outstanding", () => {
  assert.equal(backgroundSyncWork({ ...ready, hasUnsentChanges: true }, 0), 1);
  assert.equal(backgroundSyncWork({ ...ready, syncingNow: true }, 2), 3);
  assert.equal(backgroundSyncWork(ready, 1), 1);
});

test("releases background time once sync has settled", () => {
  assert.equal(backgroundSyncWork(ready, 0), 0);
});

test("does not hold background time for a failing or inactive runtime", () => {
  assert.equal(
    backgroundSyncWork(
      { ...ready, hasUnsentChanges: true, consecutiveFailures: 2 },
      0,
    ),
    0,
  );
  assert.equal(
    backgroundSyncWork({ ...ready, phase: "error", hasUnsentChanges: true }, 3),
    0,
  );
  assert.equal(backgroundSyncWork({ ...ready, running: false }, 1), 0);
});

test("reports a background sync failure only while changes are unsent", () => {
  const failing = { ...ready, hasUnsentChanges: true, consecutiveFailures: 1 };
  assert.equal(backgroundSyncFailed(failing), true);
  assert.equal(
    backgroundSyncFailed({
      ...ready,
      hasUnsentChanges: true,
      errorMessage: "offline",
    }),
    true,
  );
  assert.equal(
    backgroundSyncFailed({ ...failing, hasUnsentChanges: false }),
    false,
  );
  assert.equal(
    backgroundSyncFailed({ ...ready, hasUnsentChanges: true }),
    false,
  );
  assert.equal(backgroundSyncFailed({ ...failing, phase: "starting" }), false);
});
