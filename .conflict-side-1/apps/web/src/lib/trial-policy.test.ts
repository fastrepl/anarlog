import assert from "node:assert/strict";
import test from "node:test";

import { PRO_TRIAL_DAYS } from "@hypr/pricing/trial";

import { WEB_TRIAL_CHECKOUT_FIELDS } from "./trial-policy.ts";

test("web checkout sends the card-required shared trial policy to Stripe", () => {
  assert.deepEqual(WEB_TRIAL_CHECKOUT_FIELDS, {
    payment_method_collection: "always",
    subscription_data: {
      trial_period_days: PRO_TRIAL_DAYS,
      trial_settings: {
        end_behavior: {
          missing_payment_method: "cancel",
        },
      },
    },
  });
  assert.equal(PRO_TRIAL_DAYS, 21);
});
