import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { withDatabaseRetry } from "../database-retry";
import { createStripeSync } from "./create-stripe-sync";

describe("createStripeSync", () => {
  it("revalidates a subscription again when its database write recovers from a restart", async () => {
    const sync = createStripeSync({
      databaseUrl: "postgres://localhost/stripe_sync_test",
      stripeApiVersion: "2026-02-25.clover",
      stripeSecretKey: "test-secret",
      stripeWebhookSecret: "test-webhook-secret",
    });
    const eventCreated = 1_700_000_000;
    const webhookSubscription = {
      id: "sub_test",
      object: "subscription",
      status: "trialing",
    } as Stripe.Subscription;
    let currentSubscription = {
      ...webhookSubscription,
      status: "active",
    } as Stripe.Subscription;
    const upserts: Array<{
      subscriptions: Stripe.Subscription[];
      syncTimestamp?: string;
    }> = [];
    let recovering = true;

    sync.stripe.subscriptions.retrieve = (async (_id: string) => {
      return currentSubscription;
    }) as typeof sync.stripe.subscriptions.retrieve;
    sync.upsertSubscriptions = (async (
      subscriptions: Stripe.Subscription[],
      _backfillRelatedEntities?: boolean,
      syncTimestamp?: string,
    ) => {
      if (recovering) {
        recovering = false;
        currentSubscription = {
          ...currentSubscription,
          cancel_at_period_end: true,
        };
        throw Object.assign(new Error("database is recovering"), {
          code: "57P03",
        });
      }
      upserts.push({ subscriptions, syncTimestamp });
      return subscriptions;
    }) as typeof sync.upsertSubscriptions;

    try {
      await withDatabaseRetry(() =>
        sync.processEvent({
          id: "evt_subscription_updated",
          type: "customer.subscription.updated",
          created: eventCreated,
          data: { object: webhookSubscription },
        } as Stripe.Event),
      );
    } finally {
      await sync.postgresClient.pool.end();
    }

    expect(upserts[0]?.subscriptions).toEqual([currentSubscription]);
    expect(upserts[0]?.subscriptions[0]?.cancel_at_period_end).toBe(true);
    expect(new Date(upserts[0]?.syncTimestamp ?? 0).getTime()).toBeGreaterThan(
      eventCreated * 1_000,
    );
  });
});
