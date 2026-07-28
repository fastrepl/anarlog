import Stripe from "stripe";

import {
  getCustomerId,
  getStripeCustomer,
  getUserIdFromCustomer,
} from "./billing-bridge";
import { env } from "./env";
import { buildTrialEndingEmail } from "./trial-email-payload";

const LOOPS_TRANSACTIONAL_URL = "https://app.loops.so/api/v1/transactional";

export type TrialEndingEmailReceipt = {
  transactionalId: string;
  trialEnd: number;
  userId: string | null;
};

export async function sendTrialEndingEmail(event: Stripe.Event) {
  if (event.type !== "customer.subscription.trial_will_end") {
    return null;
  }

  if (!env.LOOPS_API_KEY || !env.LOOPS_TRIAL_ENDING_TRANSACTIONAL_ID) {
    return null;
  }

  const subscription = event.data.object as Stripe.Subscription;
  const customerId = getCustomerId(subscription);
  if (!customerId) {
    return null;
  }

  const customer = await getStripeCustomer(customerId);
  if (!customer) {
    return null;
  }

  const payload = buildTrialEndingEmail({
    subscription,
    customer,
    now: Date.now(),
  });
  if (!payload) {
    return null;
  }

  const response = await fetch(LOOPS_TRANSACTIONAL_URL, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.LOOPS_API_KEY}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      transactionalId: env.LOOPS_TRIAL_ENDING_TRANSACTIONAL_ID,
      email: payload.email,
      dataVariables: payload.dataVariables,
      // Loops takes idempotency in the body, not an Idempotency-Key header;
      // this is what dedupes webhook retries into a single email.
      idempotencyKey: event.id,
    }),
  });

  if (!response.ok) {
    throw new Error(
      `Loops transactional send failed (${response.status}): ${await response.text()}`,
    );
  }

  return {
    transactionalId: env.LOOPS_TRIAL_ENDING_TRANSACTIONAL_ID,
    trialEnd: subscription.trial_end!,
    userId: getUserIdFromCustomer(customer),
  } satisfies TrialEndingEmailReceipt;
}
