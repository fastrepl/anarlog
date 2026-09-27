import assert from "node:assert/strict";
import test from "node:test";
import type Stripe from "stripe";

import { SYNC_DEVICE_ADDON_LOOKUP_KEYS } from "./sync-device-addon.ts";
import {
  switchPlanWithAddon,
  updateDeviceAddonQuantity,
} from "./sync-device-billing.ts";

function fixture(withAddon = true, paymentError?: Error) {
  const calls: Array<{ method: string; args: unknown[] }> = [];
  const record =
    (method: string, result: unknown = {}, payment = false) =>
    async (...args: unknown[]) => {
      calls.push({ method, args });
      if (payment && paymentError) throw paymentError;
      return result;
    };
  const stripe = {
    prices: {
      list: record("prices.list", { data: [{ id: "price_addon_target" }] }),
    },
    invoices: {
      createPreview: record("invoices.createPreview", {
        amount_due: 1234,
        currency: "usd",
      }),
    },
    subscriptions: { update: record("subscriptions.update", {}, true) },
    subscriptionItems: {
      create: record("items.create", {}, true),
      update: record("items.update", {}, true),
      del: record("items.del"),
    },
  } as unknown as Stripe;
  const subscription = {
    id: "sub_pro",
    status: "active",
    items: {
      data: [
        ...(withAddon
          ? [
              {
                id: "si_addon",
                quantity: 3,
                price: {
                  id: "price_addon",
                  lookup_key: SYNC_DEVICE_ADDON_LOOKUP_KEYS.monthly,
                },
              },
            ]
          : []),
        {
          id: "si_base",
          quantity: 1,
          price: { id: "price_base", lookup_key: null },
        },
      ],
    },
  } as unknown as Stripe.Subscription;
  return { stripe, subscription, calls };
}

test("plan switch only previews until the user confirms", async () => {
  const f = fixture();
  const result = await switchPlanWithAddon({
    ...f,
    period: "yearly",
    targetPriceId: "price_yearly",
    confirmed: false,
    returnUrl: "/app/account",
  });
  assert.deepEqual(result, {
    url: null,
    confirmation: { amountDue: 1234, currency: "usd" },
  });
  assert.deepEqual(
    f.calls.map((c) => c.method),
    ["prices.list", "invoices.createPreview"],
  );
  assert.deepEqual(f.calls[1].args, [
    {
      subscription: "sub_pro",
      subscription_details: {
        items: [
          { id: "si_base", price: "price_yearly", quantity: 1 },
          { id: "si_addon", price: "price_addon_target", quantity: 3 },
        ],
        proration_behavior: "always_invoice",
      },
    },
  ]);
});

for (const period of ["monthly", "yearly"] as const) {
  test(`confirmed ${period} switch updates both items in one payment-safe request`, async () => {
    const f = fixture();
    const result = await switchPlanWithAddon({
      ...f,
      period,
      targetPriceId: "price_target",
      confirmed: true,
      returnUrl: "/app/account",
    });
    assert.equal(result.url, "/app/account");
    assert.deepEqual(f.calls, [
      {
        method: "prices.list",
        args: [
          {
            lookup_keys: [SYNC_DEVICE_ADDON_LOOKUP_KEYS[period]],
            active: true,
            limit: 1,
          },
        ],
      },
      {
        method: "subscriptions.update",
        args: [
          "sub_pro",
          {
            items: [
              { id: "si_base", price: "price_target", quantity: 1 },
              { id: "si_addon", price: "price_addon_target", quantity: 3 },
            ],
            proration_behavior: "always_invoice",
            payment_behavior: "error_if_incomplete",
          },
        ],
      },
    ]);
  });
}

test("a rejected plan payment is not returned as success", async () => {
  const error = new Error("authentication_required");
  const f = fixture(true, error);
  await assert.rejects(
    switchPlanWithAddon({
      ...f,
      period: "yearly",
      targetPriceId: "price_target",
      confirmed: true,
      returnUrl: "/app/account",
    }),
    error,
  );
});

for (const withAddon of [false, true]) {
  test(`adding slots ${withAddon ? "updates" : "creates"} with payment protection`, async () => {
    const f = fixture(withAddon);
    await updateDeviceAddonQuantity({ ...f, period: "monthly", quantity: 4 });
    const call = f.calls.at(-1)!;
    assert.equal(call.method, withAddon ? "items.update" : "items.create");
    const params = call.args[
      withAddon ? 1 : 0
    ] as Stripe.SubscriptionItemUpdateParams;
    assert.equal(params.quantity, 4);
    assert.equal(params.payment_behavior, "error_if_incomplete");
    assert.equal(params.proration_behavior, "always_invoice");
  });
  test(`failed ${withAddon ? "update" : "creation"} propagates the payment failure`, async () => {
    const error = new Error("card_declined");
    const f = fixture(withAddon, error);
    await assert.rejects(
      updateDeviceAddonQuantity({ ...f, period: "monthly", quantity: 4 }),
      error,
    );
  });
}

test("removing some or all slots retains next-invoice credits", async () => {
  for (const quantity of [2, 0]) {
    const f = fixture();
    await updateDeviceAddonQuantity({ ...f, period: "monthly", quantity });
    const call = f.calls[0];
    assert.equal(call.method, quantity ? "items.update" : "items.del");
    assert.equal(
      (call.args[1] as Stripe.SubscriptionItemUpdateParams).proration_behavior,
      "create_prorations",
    );
  }
});

test("unchanged quantity does not call Stripe", async () => {
  const f = fixture();
  await updateDeviceAddonQuantity({ ...f, period: "monthly", quantity: 3 });
  assert.deepEqual(f.calls, []);
});
