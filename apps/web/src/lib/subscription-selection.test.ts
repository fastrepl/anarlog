import assert from "node:assert/strict";
import test from "node:test";
import type Stripe from "stripe";

import {
  getPlanSwitchRoute,
  selectCurrentSubscription,
  selectPersonalPlanReplacement,
} from "./subscription-selection.ts";

const subscription = (id: string, status: Stripe.Subscription.Status) => ({
  id,
  status,
});

test("selects an active subscription before trialing or paused subscriptions", () => {
  assert.equal(
    selectCurrentSubscription([
      subscription("sub_paused", "paused"),
      subscription("sub_trial", "trialing"),
      subscription("sub_active", "active"),
    ])?.id,
    "sub_active",
  );
});

test("keeps a paused cardless trial reusable", () => {
  assert.equal(
    selectCurrentSubscription([
      subscription("sub_canceled", "canceled"),
      subscription("sub_paused", "paused"),
    ])?.id,
    "sub_paused",
  );
});

test("preserves the selected paused subscription and its billing period", () => {
  const paused = {
    id: "sub_paused",
    status: "paused",
    items: {
      data: [{ id: "si_pro", price: { id: "price_yearly" } }],
    },
  } as Stripe.Subscription;

  const selected = selectCurrentSubscription([paused]);

  assert.equal(selected, paused);
  assert.equal(selected?.items.data[0]?.price.id, "price_yearly");
});

test("routes paused plan switches to the resume portal", () => {
  const paused = {
    id: "sub_paused",
    status: "paused",
    items: {
      data: [{ id: "si_pro", price: { id: "price_yearly" } }],
    },
  } as Stripe.Subscription;

  assert.equal(getPlanSwitchRoute(paused, "price_monthly"), "portal");
});

test("routes active plan switches by their available item and price", () => {
  const active = {
    id: "sub_active",
    status: "active",
    items: {
      data: [{ id: "si_pro", price: { id: "price_monthly" } }],
    },
  } as Stripe.Subscription;

  assert.equal(getPlanSwitchRoute(active, "price_monthly"), "portal");
  assert.equal(getPlanSwitchRoute(active, "price_yearly"), "update");
  const withoutItem = {
    ...active,
    items: { ...active.items, data: [] },
  } as Stripe.Subscription;
  assert.equal(getPlanSwitchRoute(withoutItem, "price_yearly"), "checkout");
});

test("ignores subscriptions that cannot be reused", () => {
  assert.equal(
    selectCurrentSubscription([
      subscription("sub_canceled", "canceled"),
      subscription("sub_expired", "incomplete_expired"),
    ]),
    null,
  );
});

test("selects personal plans that Stripe may still renew", () => {
  const subscriptions = [
    { ...subscription("sub_unpaid", "unpaid"), cancel_at_period_end: false },
    {
      ...subscription("sub_past_due", "past_due"),
      cancel_at_period_end: false,
    },
  ];

  assert.equal(
    selectPersonalPlanReplacement(subscriptions)?.id,
    "sub_past_due",
  );
});

test("ignores paused and already-ending personal plans", () => {
  assert.equal(
    selectPersonalPlanReplacement([
      { ...subscription("sub_paused", "paused"), cancel_at_period_end: false },
      { ...subscription("sub_ending", "active"), cancel_at_period_end: true },
      {
        ...subscription("sub_canceled", "canceled"),
        cancel_at_period_end: false,
      },
    ]),
    null,
  );
});
