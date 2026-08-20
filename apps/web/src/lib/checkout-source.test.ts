import assert from "node:assert/strict";
import test from "node:test";

import {
  buildBillingRefreshDeeplink,
  checkoutSourceSchema,
} from "./checkout-source.ts";

test("keeps mobile checkout attribution through validation", () => {
  assert.equal(checkoutSourceSchema.parse("mobile"), "mobile");
  assert.equal(checkoutSourceSchema.parse("yc_perk"), "yc_perk");
  assert.equal(
    checkoutSourceSchema.catch("unknown").parse("invalid"),
    "unknown",
  );
});

test("returns checkout outcome and attribution to the app", () => {
  assert.equal(
    buildBillingRefreshDeeplink({
      scheme: "anarlog",
      checkout: "paid",
      source: "mobile",
    }),
    "anarlog://billing/refresh?checkout=paid&source=mobile",
  );

  assert.equal(
    buildBillingRefreshDeeplink({
      scheme: "anarlog-staging",
      checkout: "canceled",
      checkoutType: "trial",
      source: "mobile",
    }),
    "anarlog-staging://billing/refresh?checkout=canceled&checkout_type=trial&source=mobile",
  );

  assert.equal(
    buildBillingRefreshDeeplink({ scheme: "anarlog", source: "mobile" }),
    "anarlog://billing/refresh?source=mobile",
  );
});
