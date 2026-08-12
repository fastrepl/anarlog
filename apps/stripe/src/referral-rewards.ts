import type Stripe from "stripe";

type PreparedReward = {
  referral_id: string;
  referrer_user_id: string;
  referrer_customer_id: string;
  reward_amount_cents: number;
  reward_currency: string;
};

type ReferralRewardDependencies = {
  getReferredUserId: (customerId: string) => Promise<string | null>;
  prepareReward: (
    referredUserId: string,
    invoiceId: string,
  ) => Promise<PreparedReward | null>;
  createCredit: (reward: PreparedReward, invoiceId: string) => Promise<string>;
  completeReward: (
    referralId: string,
    invoiceId: string,
    balanceTransactionId: string,
  ) => Promise<boolean>;
};

export async function issueReferralReward(
  event: Stripe.Event,
  dependencies?: ReferralRewardDependencies,
) {
  if (event.type !== "invoice.paid") {
    return null;
  }

  const invoice = event.data.object as Stripe.Invoice;
  if (invoice.amount_paid <= 0) {
    return null;
  }

  const customerId =
    typeof invoice.customer === "string"
      ? invoice.customer
      : invoice.customer?.id;
  if (!customerId) {
    return null;
  }

  const activeDependencies =
    dependencies ?? (await createDefaultDependencies());
  const referredUserId = await activeDependencies.getReferredUserId(customerId);
  if (!referredUserId) {
    return null;
  }

  const reward = await activeDependencies.prepareReward(
    referredUserId,
    invoice.id,
  );
  if (!reward) {
    return null;
  }
  if (invoice.currency !== reward.reward_currency) {
    throw new Error("Referral reward currency does not match paid invoice");
  }

  const balanceTransactionId = await activeDependencies.createCredit(
    reward,
    invoice.id,
  );
  const completed = await activeDependencies.completeReward(
    reward.referral_id,
    invoice.id,
    balanceTransactionId,
  );
  if (!completed) {
    throw new Error("Referral reward could not be completed");
  }

  return {
    referralId: reward.referral_id,
    referrerUserId: reward.referrer_user_id,
    amount: reward.reward_amount_cents,
    currency: reward.reward_currency,
  };
}

async function createDefaultDependencies(): Promise<ReferralRewardDependencies> {
  const [billingBridge, stripeIntegration, supabaseIntegration] =
    await Promise.all([
      import("./billing-bridge"),
      import("./integration/stripe"),
      import("./integration/supabase"),
    ]);

  return {
    async getReferredUserId(customerId) {
      const customer = await billingBridge.getStripeCustomer(customerId);
      return customer ? billingBridge.getUserIdFromCustomer(customer) : null;
    },
    async prepareReward(referredUserId, invoiceId) {
      const { data, error } = await supabaseIntegration.supabaseAdmin.rpc(
        "prepare_referral_reward",
        {
          p_referred_user_id: referredUserId,
          p_invoice_id: invoiceId,
        },
      );
      if (error) {
        throw error;
      }
      return ((data ?? []) as PreparedReward[])[0] ?? null;
    },
    async createCredit(reward, invoiceId) {
      const transaction =
        await stripeIntegration.stripe.customers.createBalanceTransaction(
          reward.referrer_customer_id,
          {
            amount: -reward.reward_amount_cents,
            currency: reward.reward_currency,
            description: "Anarlog referral reward",
            metadata: {
              referral_id: reward.referral_id,
              qualifying_invoice_id: invoiceId,
              referrer_user_id: reward.referrer_user_id,
            },
          },
          { idempotencyKey: `referral-reward:${reward.referral_id}` },
        );
      return transaction.id;
    },
    async completeReward(referralId, invoiceId, balanceTransactionId) {
      const { data, error } = await supabaseIntegration.supabaseAdmin.rpc(
        "complete_referral_reward",
        {
          p_referral_id: referralId,
          p_invoice_id: invoiceId,
          p_balance_transaction_id: balanceTransactionId,
        },
      );
      if (error) {
        throw error;
      }
      return data === true;
    },
  };
}
