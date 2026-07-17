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

  it("tracks trial activation only when status changes", () => {
    const payload = getBillingAnalyticsPayload(
      event(
        "customer.subscription.updated",
        {
          status: "active",
          metadata: {},
          cancel_at_period_end: false,
          trial_end: 123,
          items: { data: [] },
        },
        { status: "trialing" },
      ),
    );

    expect(payload?.event).toBe("subscription_activated");
  });

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
