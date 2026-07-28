import { PostHog } from "posthog-node";
import Stripe from "stripe";

import { getBillingAnalyticsPayload } from "./analytics-payload";
import {
  getCustomerId,
  getStripeCustomer,
  getUserIdFromCustomer,
} from "./billing-bridge";
import { env } from "./env";

const posthog = env.POSTHOG_API_KEY
  ? new PostHog(env.POSTHOG_API_KEY, {
      host: "https://us.i.posthog.com",
      flushAt: 1,
      flushInterval: 0,
    })
  : null;

export async function captureBillingEvent(event: Stripe.Event) {
  if (!posthog) {
    return;
  }

  const payload = getBillingAnalyticsPayload(event);
  if (!payload) {
    return;
  }

  const customerId = getCustomerId(event.data.object);
  if (!customerId) {
    return;
  }

  const customer = await getStripeCustomer(customerId);
  if (!customer) {
    return;
  }

  const userId = getUserIdFromCustomer(customer);
  if (!userId) {
    return;
  }

  posthog.capture({
    distinctId: userId,
    event: payload.event,
    groups: {
      account: userId,
    },
    timestamp: new Date(event.created * 1000),
    properties: {
      ...payload.properties,
      $insert_id: `stripe-event:${event.id}`,
      source: "stripe",
      stripe_event_id: event.id,
    },
  });
  await posthog.flush();
}

export async function captureTrialEndingEmailSent({
  eventCreated,
  eventId,
  transactionalId,
  trialEnd,
  userId,
}: {
  eventCreated: number;
  eventId: string;
  transactionalId: string;
  trialEnd: number;
  userId: string | null;
}) {
  if (!posthog || !userId) {
    return;
  }

  posthog.capture({
    distinctId: userId,
    event: "trial_ending_email_sent",
    groups: {
      account: userId,
    },
    timestamp: new Date(eventCreated * 1000),
    properties: {
      $insert_id: `stripe-event:${eventId}:trial-ending-email-sent`,
      source: "loops",
      transactional_id: transactionalId,
      trial_end: trialEnd,
      days_until_trial_end: Math.max(
        0,
        Math.ceil((trialEnd - eventCreated) / 86_400),
      ),
    },
  });
  await posthog.flush();
}
