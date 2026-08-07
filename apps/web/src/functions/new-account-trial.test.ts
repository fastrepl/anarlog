import assert from "node:assert/strict";
import test from "node:test";

import { ensureNewAccountTrial } from "./new-account-trial.ts";

test("accepts a newly started trial", async () => {
  await assert.doesNotReject(() =>
    ensureNewAccountTrial("access-token", async () => ({
      data: { started: true, reason: "started" },
    })),
  );
});

test("accepts an account that is already ineligible", async () => {
  await assert.doesNotReject(() =>
    ensureNewAccountTrial("access-token", async () => ({
      data: { started: false, reason: "not_eligible" },
    })),
  );
});

test("rejects API and malformed responses", async () => {
  await assert.rejects(
    ensureNewAccountTrial("access-token", async () => ({
      error: new Error("API unavailable"),
    })),
    /API unavailable/,
  );
  await assert.rejects(
    ensureNewAccountTrial("access-token", async () => ({
      data: { started: false },
    })),
    /invalid response/,
  );
});
