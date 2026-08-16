import { expect, test } from "bun:test";

import {
  getTeamSubscriptionSeatLimit,
  getWorkspaceBillingUpdate,
} from "./workspace-billing";

const subscriptionWithItems = (
  items: Array<{ price: { id: string }; quantity: number | null }>,
) =>
  ({
    items: { data: items },
  }) as Parameters<typeof getTeamSubscriptionSeatLimit>[0];

test("reads the configured Team subscription quantity", () => {
  expect(
    getTeamSubscriptionSeatLimit(
      subscriptionWithItems([
        { price: { id: "price_team" }, quantity: 7 },
        { price: { id: "price_addon" }, quantity: 1 },
      ]),
      new Set(["price_team"]),
    ),
  ).toBe(7);
});

test("uses Stripe's default quantity for a Team subscription item", () => {
  expect(
    getTeamSubscriptionSeatLimit(
      subscriptionWithItems([{ price: { id: "price_team" }, quantity: null }]),
      new Set(["price_team"]),
    ),
  ).toBe(1);
});

test("ignores subscriptions without a configured Team price", () => {
  expect(
    getTeamSubscriptionSeatLimit(
      subscriptionWithItems([{ price: { id: "price_personal" }, quantity: 1 }]),
      new Set(["price_team"]),
    ),
  ).toBeNull();
});

test("rejects ambiguous Team subscription items", () => {
  expect(
    getTeamSubscriptionSeatLimit(
      subscriptionWithItems([
        { price: { id: "price_team_monthly" }, quantity: 3 },
        { price: { id: "price_team_yearly" }, quantity: 3 },
      ]),
      new Set(["price_team_monthly", "price_team_yearly"]),
    ),
  ).toBeNull();
});

test("reconciles quantities from subscription lifecycle events", () => {
  const event = {
    type: "customer.subscription.updated",
    data: {
      object: subscriptionWithItems([
        { price: { id: "price_team" }, quantity: 4 },
      ]),
    },
  } as Parameters<typeof getWorkspaceBillingUpdate>[0];

  expect(getWorkspaceBillingUpdate(event, new Set(["price_team"]))).toEqual({
    seatLimit: 4,
    updateSeatLimit: true,
  });
});

test("clears the seat limit when a Team subscription is deleted", () => {
  const event = {
    type: "customer.subscription.deleted",
    data: {
      object: subscriptionWithItems([
        { price: { id: "price_team" }, quantity: 4 },
      ]),
    },
  } as Parameters<typeof getWorkspaceBillingUpdate>[0];

  expect(getWorkspaceBillingUpdate(event, new Set(["price_team"]))).toEqual({
    seatLimit: null,
    updateSeatLimit: true,
  });
});

test("binds customer events without changing the seat limit", () => {
  const event = {
    type: "customer.updated",
    data: { object: {} },
  } as Parameters<typeof getWorkspaceBillingUpdate>[0];

  expect(getWorkspaceBillingUpdate(event, new Set(["price_team"]))).toEqual({
    seatLimit: null,
    updateSeatLimit: false,
  });
});
