import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { issueReferralReward } from "./referral-rewards";

function invoiceEvent(overrides: Partial<Stripe.Invoice> = {}): Stripe.Event {
  return {
    id: "evt_referral",
    type: "invoice.paid",
    data: {
      object: {
        id: "in_referral_first",
        customer: "cus_referred",
        amount_paid: 1500,
        currency: "usd",
        ...overrides,
      } as Stripe.Invoice,
    },
  } as Stripe.Event;
}

describe("issueReferralReward", () => {
  it("ignores zero-value trial invoices", async () => {
    const result = await issueReferralReward(invoiceEvent({ amount_paid: 0 }), {
      getReferredUserId: async () => {
        throw new Error("should not resolve customer");
      },
      prepareReward: async () => null,
      createCredit: async () => "cbtxn_unused",
      completeReward: async () => true,
    });

    expect(result).toBeNull();
  });

  it("credits the referrer after the referred user's first payment", async () => {
    const calls: string[] = [];
    const result = await issueReferralReward(invoiceEvent(), {
      getReferredUserId: async (customerId) => {
        calls.push(`customer:${customerId}`);
        return "user_referred";
      },
      prepareReward: async (userId, invoiceId) => {
        calls.push(`prepare:${userId}:${invoiceId}`);
        return {
          referral_id: "referral_1",
          referrer_user_id: "user_referrer",
          referrer_customer_id: "cus_referrer",
          reward_amount_cents: 1500,
          reward_currency: "usd",
        };
      },
      createCredit: async (reward, invoiceId) => {
        calls.push(
          `credit:${reward.referrer_customer_id}:${reward.reward_amount_cents}:${invoiceId}`,
        );
        return "cbtxn_referral";
      },
      completeReward: async (referralId, invoiceId, transactionId) => {
        calls.push(`complete:${referralId}:${invoiceId}:${transactionId}`);
        return true;
      },
    });

    expect(result).toEqual({
      referralId: "referral_1",
      referrerUserId: "user_referrer",
      amount: 1500,
      currency: "usd",
    });
    expect(calls).toEqual([
      "customer:cus_referred",
      "prepare:user_referred:in_referral_first",
      "credit:cus_referrer:1500:in_referral_first",
      "complete:referral_1:in_referral_first:cbtxn_referral",
    ]);
  });

  it("fails closed when the paid invoice currency differs", async () => {
    await expect(
      issueReferralReward(invoiceEvent({ currency: "eur" }), {
        getReferredUserId: async () => "user_referred",
        prepareReward: async () => ({
          referral_id: "referral_1",
          referrer_user_id: "user_referrer",
          referrer_customer_id: "cus_referrer",
          reward_amount_cents: 1500,
          reward_currency: "usd",
        }),
        createCredit: async () => "cbtxn_unused",
        completeReward: async () => true,
      }),
    ).rejects.toThrow("Referral reward currency does not match paid invoice");
  });
});
