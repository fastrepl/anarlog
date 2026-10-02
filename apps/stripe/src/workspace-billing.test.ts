import { expect, test } from "bun:test";

import {
  getWorkspaceBillingUpdate,
  getWorkspaceSubscriptionSeatLimit,
} from "./workspace-billing";

const subscriptionWithItems = (
  items: readonly { price: { id: string }; quantity: number | null }[],
) =>
  ({
    items: { data: [...items] },
  }) as Parameters<typeof getWorkspaceSubscriptionSeatLimit>[0];

test.each([
  [
    "shared Pro subscription quantity",
    [{ price: { id: "price_pro" }, quantity: 7 }],
    7,
  ],
  [
    "Stripe default quantity",
    [{ price: { id: "price_pro" }, quantity: null }],
    1,
  ],
  ["subscription without a price item", [], null],
  [
    "ambiguous subscription items",
    [
      { price: { id: "price_pro" }, quantity: 3 },
      { price: { id: "price_addon" }, quantity: 1 },
    ],
    null,
  ],
] as const)("gets the seat limit for a %s", (_label, items, expected) => {
  expect(getWorkspaceSubscriptionSeatLimit(subscriptionWithItems(items))).toBe(
    expected,
  );
});

test.each([
  [
    "customer.subscription.deleted",
    subscriptionWithItems([{ price: { id: "price_pro" }, quantity: 4 }]),
    { seatLimit: null, updateSeatLimit: true },
  ],
  ["customer.updated", {}, { seatLimit: null, updateSeatLimit: false }],
] as const)(
  "maps %s to its workspace billing update",
  (type, object, expected) => {
    const event = {
      type,
      data: { object },
    } as Parameters<typeof getWorkspaceBillingUpdate>[0];

    expect(getWorkspaceBillingUpdate(event)).toEqual(expected);
  },
);
