import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { buildTrialEndingEmail } from "./trial-email-payload";

const NOW = Date.UTC(2026, 6, 30);
const TRIAL_END = Math.floor(Date.UTC(2026, 7, 2) / 1000);

const subscription = (overrides: Record<string, unknown> = {}) =>
  ({
    trial_end: TRIAL_END,
    default_payment_method: null,
    ...overrides,
  }) as unknown as Stripe.Subscription;

const customer = (overrides: Record<string, unknown> = {}) =>
  ({
    email: "user@example.com",
    invoice_settings: { default_payment_method: null },
    default_source: null,
    ...overrides,
  }) as unknown as Stripe.Customer;

describe("buildTrialEndingEmail", () => {
  it("builds a reminder for a cardless trial", () => {
    const payload = buildTrialEndingEmail({
      subscription: subscription(),
      customer: customer(),
      now: NOW,
    });

    expect(payload).toEqual({
      email: "user@example.com",
      dataVariables: {
        daysRemaining: 3,
        trialEndDate: "August 2, 2026",
      },
    });
  });

  it("skips card-backed trials", () => {
    expect(
      buildTrialEndingEmail({
        subscription: subscription({ default_payment_method: "pm_123" }),
        customer: customer(),
        now: NOW,
      }),
    ).toBeNull();

    expect(
      buildTrialEndingEmail({
        subscription: subscription(),
        customer: customer({
          invoice_settings: { default_payment_method: "pm_123" },
        }),
        now: NOW,
      }),
    ).toBeNull();

    expect(
      buildTrialEndingEmail({
        subscription: subscription(),
        customer: customer({ default_source: "card_123" }),
        now: NOW,
      }),
    ).toBeNull();
  });

  it("skips customers without an email", () => {
    expect(
      buildTrialEndingEmail({
        subscription: subscription(),
        customer: customer({ email: null }),
        now: NOW,
      }),
    ).toBeNull();
  });

  it("skips trials that already ended", () => {
    expect(
      buildTrialEndingEmail({
        subscription: subscription({ trial_end: Math.floor(NOW / 1000) - 60 }),
        customer: customer(),
        now: NOW,
      }),
    ).toBeNull();
  });
});
