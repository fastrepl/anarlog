import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { getBillingAnalyticsPayload } from "./analytics-payload";

const event = (
  type: Stripe.Event.Type,
  object: Record<string, unknown>,
  previousAttributes?: Record<string, unknown>,
) =>
  ({
    id: "evt_test",
    type,
    data: { object, previous_attributes: previousAttributes },
  }) as unknown as Stripe.Event;

describe("getBillingAnalyticsPayload", () => {
  const subscription = {
    status: "active",
    metadata: {},
    cancel_at_period_end: false,
    trial_end: null,
    items: { data: [] },
  };

  it("tracks a trial created from checkout", () => {
    const payload = getBillingAnalyticsPayload(
      event("customer.subscription.created", {
        status: "trialing",
        metadata: { checkout_type: "trial", source: "settings" },
        cancel_at_period_end: false,
        trial_end: 123,
        items: {
          data: [
            {
              price: {
                id: "price_pro",
                recurring: { interval: "month" },
              },
            },
          ],
        },
      }),
    );

    expect(payload).toEqual({
      event: "trial_started",
      properties: {
        plan: "pro",
        status: "trialing",
        interval: "month",
        price_id: "price_pro",
        checkout_type: "trial",
        entry_source: "settings",
        cancel_at_period_end: false,
        trial_end: 123,
      },
    });
  });

  it.each([
    [{ status: "trialing" }, false, "subscription_activated"],
    [{ cancel_at_period_end: false }, true, "subscription_cancel_scheduled"],
    [{ cancel_at_period_end: true }, false, "subscription_resumed"],
    [{ items: { data: [] } }, false, "subscription_plan_changed"],
  ] as const)(
    "classifies subscription updates as %s",
    (previousAttributes, cancelAtPeriodEnd, expectedEvent) => {
      const payload = getBillingAnalyticsPayload(
        event(
          "customer.subscription.updated",
          { ...subscription, cancel_at_period_end: cancelAtPeriodEnd },
          previousAttributes,
        ),
      );

      expect(payload?.event).toBe(expectedEvent);
    },
  );

  it("ignores zero-dollar trial invoices", () => {
    expect(
      getBillingAnalyticsPayload(
        event("invoice.paid", {
          amount_paid: 0,
          amount_due: 0,
          currency: "usd",
          billing_reason: "subscription_create",
          attempt_count: 0,
        }),
      ),
    ).toBeNull();
  });
});
