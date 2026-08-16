import { expect, test } from "bun:test";

import {
  getWorkspaceBillingUpdate,
  getWorkspaceSubscriptionSeatLimit,
} from "./workspace-billing";

const subscriptionWithItems = (
  items: Array<{ price: { id: string }; quantity: number | null }>,
) =>
  ({
    items: { data: items },
  }) as Parameters<typeof getWorkspaceSubscriptionSeatLimit>[0];

test("reads the shared Pro subscription quantity as workspace seats", () => {
  expect(
    getWorkspaceSubscriptionSeatLimit(
      subscriptionWithItems([{ price: { id: "price_pro" }, quantity: 7 }]),
    ),
  ).toBe(7);
});

test("uses Stripe's default quantity for a workspace subscription item", () => {
  expect(
    getWorkspaceSubscriptionSeatLimit(
      subscriptionWithItems([{ price: { id: "price_pro" }, quantity: null }]),
    ),
  ).toBe(1);
});

test("rejects workspace subscriptions without a price item", () => {
  expect(
    getWorkspaceSubscriptionSeatLimit(subscriptionWithItems([])),
  ).toBeNull();
});

test("rejects ambiguous workspace subscription items", () => {
  expect(
    getWorkspaceSubscriptionSeatLimit(
      subscriptionWithItems([
        { price: { id: "price_pro" }, quantity: 3 },
        { price: { id: "price_addon" }, quantity: 1 },
      ]),
    ),
  ).toBeNull();
});

test("reconciles quantities from subscription lifecycle events", () => {
  const event = {
    type: "customer.subscription.updated",
    data: {
      object: subscriptionWithItems([
        { price: { id: "price_pro" }, quantity: 4 },
      ]),
    },
  } as Parameters<typeof getWorkspaceBillingUpdate>[0];

  expect(getWorkspaceBillingUpdate(event)).toEqual({
    seatLimit: 4,
    updateSeatLimit: true,
  });
});

test("clears the seat limit when a Team subscription is deleted", () => {
  const event = {
    type: "customer.subscription.deleted",
    data: {
      object: subscriptionWithItems([
        { price: { id: "price_pro" }, quantity: 4 },
      ]),
    },
  } as Parameters<typeof getWorkspaceBillingUpdate>[0];

  expect(getWorkspaceBillingUpdate(event)).toEqual({
    seatLimit: null,
    updateSeatLimit: true,
  });
});

test("binds customer events without changing the seat limit", () => {
  const event = {
    type: "customer.updated",
    data: { object: {} },
  } as Parameters<typeof getWorkspaceBillingUpdate>[0];

  expect(getWorkspaceBillingUpdate(event)).toEqual({
    seatLimit: null,
    updateSeatLimit: false,
  });
});
