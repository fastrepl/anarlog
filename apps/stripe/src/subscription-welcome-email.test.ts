import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import {
  sendSubscriptionWelcomeEmail,
  type SubscriptionWelcomeEmailDependencies,
} from "./subscription-welcome-email";

function invoiceEvent(overrides: Partial<Stripe.Invoice> = {}): Stripe.Event {
  return {
    id: "evt_first_subscription_payment",
    type: "invoice.paid",
    data: {
      object: {
        id: "in_first_subscription_payment",
        amount_paid: 1500,
        billing_reason: "subscription_create",
        created: 1_786_500_000,
        customer: "cus_subscriber",
        parent: {
          type: "subscription_details",
          quote_details: null,
          subscription_details: {
            metadata: null,
            subscription: "sub_anarlog_pro",
          },
        },
        ...overrides,
      } as Stripe.Invoice,
    },
  } as Stripe.Event;
}

function dependencies(
  overrides: Partial<SubscriptionWelcomeEmailDependencies> = {},
): SubscriptionWelcomeEmailDependencies {
  return {
    apiKey: "loops-key",
    getCustomer: async () =>
      ({
        id: "cus_subscriber",
        email: "subscriber@example.com",
        name: "Ada Lovelace",
      }) as Stripe.Customer,
    hasEarlierPaidSubscriptionInvoice: async () => false,
    sendTransactional: async () => {},
    ...overrides,
  };
}

describe("sendSubscriptionWelcomeEmail", () => {
  it("sends after the first paid subscription invoice", async () => {
    const sends: Array<Record<string, unknown>> = [];

    const result = await sendSubscriptionWelcomeEmail(
      invoiceEvent(),
      dependencies({
        sendTransactional: async (payload) => {
          sends.push(payload);
        },
      }),
    );

    expect(result).toEqual({
      invoiceId: "in_first_subscription_payment",
      transactionalId: "cmsq3t8ns0ffi0jydc6uzj1rt",
    });
    expect(sends).toEqual([
      {
        apiKey: "loops-key",
        transactionalId: "cmsq3t8ns0ffi0jydc6uzj1rt",
        email: "subscriber@example.com",
        dataVariables: {},
        idempotencyKey: "evt_first_subscription_payment",
      },
    ]);
  });

  it("sends when a trial converts on its first positive invoice", async () => {
    let sent = false;

    await sendSubscriptionWelcomeEmail(
      invoiceEvent({ billing_reason: "subscription_cycle" }),
      dependencies({
        sendTransactional: async () => {
          sent = true;
        },
      }),
    );

    expect(sent).toBeTrue();
  });

  it("does not send on a renewal", async () => {
    const result = await sendSubscriptionWelcomeEmail(
      invoiceEvent({ billing_reason: "subscription_cycle" }),
      dependencies({
        hasEarlierPaidSubscriptionInvoice: async () => true,
        sendTransactional: async () => {
          throw new Error("should not send");
        },
      }),
    );

    expect(result).toBeNull();
  });

  it("ignores zero-value trial invoices", async () => {
    const result = await sendSubscriptionWelcomeEmail(
      invoiceEvent({ amount_paid: 0 }),
      dependencies({
        hasEarlierPaidSubscriptionInvoice: async () => {
          throw new Error("should not inspect invoice history");
        },
      }),
    );

    expect(result).toBeNull();
  });

  it("ignores paid invoices that are not subscription-backed", async () => {
    const result = await sendSubscriptionWelcomeEmail(
      invoiceEvent({ billing_reason: "manual", parent: null }),
      dependencies({
        hasEarlierPaidSubscriptionInvoice: async () => {
          throw new Error("should not inspect invoice history");
        },
      }),
    );

    expect(result).toBeNull();
  });
});
