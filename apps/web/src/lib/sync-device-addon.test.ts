import assert from "node:assert/strict";
import test from "node:test";

import { getSubscriptionBillingPeriod } from "./subscription-selection.ts";
import {
  getSyncDeviceAddonQuantity,
  selectBasePlanItem,
  selectSyncDeviceAddonItem,
  SYNC_DEVICE_ADDON_LOOKUP_KEYS,
} from "./sync-device-addon.ts";

const addonItem = {
  id: "si_addon",
  quantity: 2,
  price: {
    id: "price_addon",
    lookup_key: SYNC_DEVICE_ADDON_LOOKUP_KEYS.monthly,
    recurring: { interval: "month" },
  },
};

const planItem = {
  id: "si_plan",
  quantity: 1,
  price: {
    id: "price_pro_yearly",
    lookup_key: null,
    recurring: { interval: "year" },
  },
};

test("add-on items are separated from the base plan item", () => {
  const items = [addonItem, planItem];
  assert.equal(selectBasePlanItem(items)?.id, "si_plan");
  assert.equal(selectSyncDeviceAddonItem(items)?.id, "si_addon");
  assert.equal(getSyncDeviceAddonQuantity(items), 2);
  assert.equal(getSyncDeviceAddonQuantity([planItem]), 0);
});

test("billing period follows the base plan even when an add-on is listed first", () => {
  assert.equal(
    getSubscriptionBillingPeriod({ items: { data: [addonItem, planItem] } }),
    "yearly",
  );
});
