import assert from "node:assert/strict";
import test from "node:test";

import { backgroundSyncWork } from "./background-work.ts";

const ready = {
  phase: "ready",
  running: true,
  syncingNow: false,
  hasUnsentChanges: false,
  consecutiveFailures: 0,
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
