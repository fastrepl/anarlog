import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { syncBillingBridge } from "./billing-bridge";

const customer = (metadata: Record<string, string>) =>
  ({ id: "cus_team123", metadata }) as Stripe.Customer;

const event = (type: Stripe.Event.Type, object: Stripe.Event.Data.Object) =>
  ({
    id: "evt_team123",
    type,
    data: { object },
  }) as Stripe.Event;

const teamSubscription = {
  customer: "cus_team123",
  items: {
    data: [{ price: { id: "price_pro" }, quantity: 4 }],
  },
} as Stripe.Subscription;

const dependencies = (
  overrides: Partial<NonNullable<Parameters<typeof syncBillingBridge>[1]>> = {},
): NonNullable<Parameters<typeof syncBillingBridge>[1]> => ({
  getCustomer: async () => customer({ workspaceId: "workspace-123" }),
  updateCustomerMetadata: async () => {
    throw new Error("should not update personal metadata");
  },
  assignProfileCustomer: async () => {
    throw new Error("should not assign a personal customer");
  },
  deleteCustomer: async () => {
    throw new Error("should not delete a workspace customer");
  },
  syncWorkspaceCustomer: async () => "cus_team123",
  ...overrides,
});

describe("syncBillingBridge", () => {
  it("syncs Team subscription quantities after the database recovers", async () => {
    const updates: Array<Record<string, unknown>> = [];
    let recovering = true;

    await syncBillingBridge(
      event("customer.subscription.updated", teamSubscription),
      dependencies({
        syncWorkspaceCustomer: async (update) => {
          if (recovering) {
            recovering = false;
            throw { code: "PGRST001" };
          }
          updates.push(update);
          return update.customerId;
        },
      }),
    );

    expect(updates).toEqual([
      {
        workspaceId: "workspace-123",
        customerId: "cus_team123",
        seatLimit: 4,
        updateSeatLimit: true,
      },
    ]);
  });

  it("fails closed when Stripe metadata conflicts with the bound customer", async () => {
    await expect(
      syncBillingBridge(
        event("customer.subscription.updated", teamSubscription),
        dependencies({
          syncWorkspaceCustomer: async () => "cus_another_workspace",
        }),
      ),
    ).rejects.toThrow("Workspace Stripe customer assignment conflict");
  });

  it("ignores Stripe events for a workspace that no longer exists", async () => {
    await expect(
      syncBillingBridge(
        event("customer.subscription.updated", teamSubscription),
        dependencies({
          syncWorkspaceCustomer: async () => null,
        }),
      ),
    ).resolves.toBeUndefined();
  });

  it("finishes personal billing sync after a transient database outage without deleting the customer", async () => {
    const metadataUpdates: Array<Record<string, string>> = [];
    const assignments: string[][] = [];
    const failures = [{ code: "PGRST002" }, { code: "", status: 503 }];

    await syncBillingBridge(
      event(
        "customer.updated",
        customer({ userId: "user-123" }) as Stripe.Event.Data.Object,
      ),
      dependencies({
        getCustomer: async () => customer({ userId: "user-123" }),
        updateCustomerMetadata: async (_customerId, metadata) => {
          metadataUpdates.push(metadata);
        },
        assignProfileCustomer: async (userId, customerId) => {
          const failure = failures.shift();
          if (failure) throw failure;
          assignments.push([userId, customerId]);
          return customerId;
        },
        syncWorkspaceCustomer: async () => {
          throw new Error("should not assign a workspace customer");
        },
      }),
    );

    expect(metadataUpdates).toEqual([
      {
        userId: "user-123",
        posthog_person_distinct_id: "user-123",
      },
    ]);
    expect(assignments).toEqual([["user-123", "cus_team123"]]);
  });

  it.each([
    { code: "23505", message: "assignment conflict" },
    { code: "57P03", message: "database is recovering" },
  ])(
    "propagates permanent or exhausted database failures ($code)",
    async (failure) => {
      let assigned = false;
      let deleted = false;
      await expect(
        syncBillingBridge(
          event("customer.subscription.updated", teamSubscription),
          dependencies({
            getCustomer: async () =>
              customer({
                userId: "user-123",
                posthog_person_distinct_id: "user-123",
              }),
            assignProfileCustomer: async () => {
              // A permanent failure must not be retried into a false success.
              if (failure.code === "23505" && assigned) return "cus_team123";
              assigned = true;
              throw failure;
            },
            deleteCustomer: async () => {
              deleted = true;
            },
          }),
        ),
      ).rejects.toBe(failure);
      expect(deleted).toBe(false);
    },
  );
});
