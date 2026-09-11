import { describe, expect, test } from "bun:test";
import type Stripe from "stripe";

import {
  reconcileWorkspaceSeatEvent,
  type WorkspaceSeatEvent,
} from "./workspace-seat-reconciliation";

const event: WorkspaceSeatEvent = {
  id: "42",
  workspace_id: "workspace-1",
  customer_id: "cus_team",
  quantity: 3,
  occurred_at: new Date(1_750_000_500_000),
};

function fixture() {
  const subscription = {
    id: "sub_team",
    status: "active",
    start_date: 1_750_000_000,
    schedule: null,
    metadata: {},
    items: {
      data: [
        {
          id: "si_team",
          quantity: 2,
          current_period_start: 1_750_000_000,
          current_period_end: 1_750_003_000,
          price: {
            recurring: {
              interval: "month",
              interval_count: 1,
              usage_type: "licensed",
            },
          },
        },
      ],
    },
  } as unknown as Stripe.Subscription;
  const customer = {
    id: "cus_team",
    metadata: { workspace_id: "workspace-1" },
  } as unknown as Stripe.Customer;
  const updates: Array<{
    id: string;
    params: Stripe.SubscriptionUpdateParams;
    options: Stripe.RequestOptions;
  }> = [];
  let failAfterUpdate = false;
  const api = {
    customers: { retrieve: async () => customer },
    subscriptions: {
      list: async () => ({ data: [subscription], has_more: false }),
      update: async (
        id: string,
        params: Stripe.SubscriptionUpdateParams,
        options: Stripe.RequestOptions,
      ) => {
        updates.push({ id, params, options });
        subscription.items.data[0].quantity = params.items![0].quantity!;
        Object.assign(subscription.metadata, params.metadata);
        if (failAfterUpdate)
          throw new Error("Connection lost after Stripe applied the update");
        return subscription;
      },
    },
  } as unknown as Pick<Stripe, "customers" | "subscriptions">;
  return {
    subscription,
    customer,
    updates,
    api,
    failAfterUpdate: () => {
      failAfterUpdate = true;
    },
  };
}

describe("membership-driven Team billing", () => {
  test("adds a member at the join time and accrues proration without an immediate invoice", async () => {
    const f = fixture();
    await reconcileWorkspaceSeatEvent(event, f.api);
    expect(f.updates).toEqual([
      {
        id: "sub_team",
        params: {
          items: [{ id: "si_team", quantity: 3 }],
          metadata: { anarlog_seat_event_id: "workspace-1:42" },
          proration_date: 1_750_000_500,
          proration_behavior: "create_prorations",
        },
        options: { idempotencyKey: "workspace-seat-workspace-1-42" },
      },
    ]);
  });

  test("removal credits unused time and reduces the next renewal quantity", async () => {
    const f = fixture();
    await reconcileWorkspaceSeatEvent({ ...event, quantity: 1 }, f.api);
    expect(f.updates[0].params.proration_behavior).toBe("create_prorations");
    expect(f.updates[0].params.items![0].quantity).toBe(1);
  });

  test("does not credit time that has not been paid for", async () => {
    const f = fixture();
    f.subscription.status = "past_due";
    await reconcileWorkspaceSeatEvent({ ...event, quantity: 1 }, f.api);
    expect(f.updates[0].params.proration_behavior).toBe("none");
  });

  test("annual plans preserve their existing renewal schedule", async () => {
    const f = fixture();
    f.subscription.items.data[0].price.recurring!.interval = "year";
    await reconcileWorkspaceSeatEvent(event, f.api);
    expect(f.updates[0].params.billing_cycle_anchor).toBeUndefined();
    expect(f.updates[0].params.proration_behavior).toBe("create_prorations");
  });

  test("a retry after a successful Stripe write cannot charge twice", async () => {
    const f = fixture();
    f.failAfterUpdate();
    await expect(reconcileWorkspaceSeatEvent(event, f.api)).rejects.toThrow(
      "Connection lost",
    );
    // Even an external quantity edit after the uncertain write cannot replay it.
    f.subscription.items.data[0].quantity = 4;
    await reconcileWorkspaceSeatEvent(event, f.api);
    expect(f.updates).toHaveLength(1);
  });

  test("a later join after a removal is a distinct event", async () => {
    const f = fixture();
    await reconcileWorkspaceSeatEvent(event, f.api);
    await reconcileWorkspaceSeatEvent(
      { ...event, id: "43", quantity: 2 },
      f.api,
    );
    await reconcileWorkspaceSeatEvent(
      { ...event, id: "44", quantity: 3 },
      f.api,
    );
    expect(f.updates.map((u) => u.params.items![0].quantity)).toEqual([
      3, 2, 3,
    ]);
    expect(new Set(f.updates.map((u) => u.options.idempotencyKey)).size).toBe(
      3,
    );
  });

  test("unchanged quantity does not create invoice items", async () => {
    const f = fixture();
    await reconcileWorkspaceSeatEvent({ ...event, quantity: 2 }, f.api);
    expect(f.updates).toHaveLength(0);
  });

  test("pre-checkout reconciliation starts at the first paid period", async () => {
    const f = fixture();
    await reconcileWorkspaceSeatEvent(
      { ...event, occurred_at: new Date(1_749_999_000_000) },
      f.api,
    );
    expect(f.updates[0].params.proration_date).toBe(f.subscription.start_date);
  });

  test("historical changes cannot silently move into a later invoice period", async () => {
    const f = fixture();
    f.subscription.items.data[0].current_period_start = 1_750_003_000;
    f.subscription.items.data[0].current_period_end = 1_750_006_000;
    await expect(reconcileWorkspaceSeatEvent(event, f.api)).rejects.toThrow(
      "historical invoice",
    );
    expect(f.updates).toHaveLength(0);
  });

  test("rejects personal or conflicting customer ownership", async () => {
    const f = fixture();
    f.customer.metadata = { user_id: "someone" };
    await expect(reconcileWorkspaceSeatEvent(event, f.api)).rejects.toThrow(
      "ownership mismatch",
    );
    expect(f.updates).toHaveLength(0);
  });

  test("does not change canceled subscriptions, multi-item plans or schedules", async () => {
    const f = fixture();
    f.subscription.status = "canceled";
    await expect(reconcileWorkspaceSeatEvent(event, f.api)).rejects.toThrow(
      "exactly one",
    );
    f.subscription.status = "active";
    f.subscription.schedule = "sub_sched";
    await expect(reconcileWorkspaceSeatEvent(event, f.api)).rejects.toThrow(
      "Unsupported",
    );
    f.subscription.schedule = null;
    f.subscription.items.data.push(f.subscription.items.data[0]);
    await expect(reconcileWorkspaceSeatEvent(event, f.api)).rejects.toThrow(
      "Unsupported",
    );
    expect(f.updates).toHaveLength(0);
  });
});
