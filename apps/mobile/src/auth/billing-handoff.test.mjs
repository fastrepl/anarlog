import assert from "node:assert/strict";
import test from "node:test";

import {
  parseBillingCallbackUrl,
  refreshBillingEntitlement,
} from "./billing-handoff.ts";

test("parses the mobile checkout return without accepting other deep links", () => {
  assert.deepEqual(
    parseBillingCallbackUrl(
      "anarlog://billing/refresh?checkout=paid&source=mobile",
    ),
    { checkout: "paid", checkoutType: null, source: "mobile" },
  );
  assert.deepEqual(
    parseBillingCallbackUrl(
      "anarlog://billing/refresh?checkout=canceled&checkout_type=trial&source=mobile",
    ),
    { checkout: "canceled", checkoutType: "trial", source: "mobile" },
  );
  assert.equal(parseBillingCallbackUrl("anarlog://auth/callback"), null);
  assert.equal(
    parseBillingCallbackUrl("attacker://billing/refresh?checkout=paid"),
    null,
  );
});

test("retries bounded entitlement refresh until Pro is visible", async () => {
  const waits = [];
  let attempts = 0;
  const unlocked = await refreshBillingEntitlement({
    delaysMs: [0, 10, 20, 30],
    wait: async (delayMs) => {
      waits.push(delayMs);
    },
    refresh: async () => {
      attempts += 1;
      return attempts === 3;
    },
  });

  assert.equal(unlocked, true);
  assert.equal(attempts, 3);
  assert.deepEqual(waits, [10, 20]);
});

test("stops after the bounded refresh window", async () => {
  let attempts = 0;
  const unlocked = await refreshBillingEntitlement({
    delaysMs: [0, 1, 2],
    wait: async () => {},
    refresh: async () => {
      attempts += 1;
      return false;
    },
  });

  assert.equal(unlocked, false);
  assert.equal(attempts, 3);
});
