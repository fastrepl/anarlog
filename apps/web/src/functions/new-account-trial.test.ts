import assert from "node:assert/strict";
import test from "node:test";

import { ensureNewAccountTrial } from "./new-account-trial.ts";

test("accepts a newly started trial", async () => {
  assert.equal(
    await ensureNewAccountTrial("access-token", async () => ({
      data: { started: true, reason: "started" },
    })),
    "started",
  );
});

test("accepts an account that is already ineligible", async () => {
  assert.equal(
    await ensureNewAccountTrial("access-token", async () => ({
      data: { started: false, reason: "not_eligible" },
    })),
    "not_eligible",
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
