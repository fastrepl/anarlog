import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { createStripeSync } from "./create-stripe-sync";

describe("createStripeSync", () => {
  it("revalidates subscription events before writing them", async () => {
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
    const currentSubscription = {
      ...webhookSubscription,
      status: "active",
    } as Stripe.Subscription;
    const retrievedIds: string[] = [];
    const upserts: Array<{
      subscriptions: Stripe.Subscription[];
      syncTimestamp?: string;
    }> = [];

    sync.stripe.subscriptions.retrieve = (async (id: string) => {
      retrievedIds.push(id);
      return currentSubscription;
    }) as typeof sync.stripe.subscriptions.retrieve;
    sync.upsertSubscriptions = (async (
      subscriptions: Stripe.Subscription[],
      _backfillRelatedEntities?: boolean,
      syncTimestamp?: string,
    ) => {
      upserts.push({ subscriptions, syncTimestamp });
      return subscriptions;
    }) as typeof sync.upsertSubscriptions;

    try {
      await sync.processEvent({
        id: "evt_subscription_updated",
        type: "customer.subscription.updated",
        created: eventCreated,
        data: { object: webhookSubscription },
      } as Stripe.Event);
    } finally {
      await sync.postgresClient.pool.end();
    }

    expect(retrievedIds).toEqual(["sub_test"]);
    expect(upserts).toHaveLength(1);
    expect(upserts[0]?.subscriptions).toEqual([currentSubscription]);
    expect(new Date(upserts[0]?.syncTimestamp ?? 0).getTime()).toBeGreaterThan(
      eventCreated * 1_000,
    );
  });
});
