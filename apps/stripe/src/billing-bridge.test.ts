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
  teamPriceIds: new Set(["price_team"]),
  ...overrides,
});

describe("syncBillingBridge", () => {
  it("routes Team subscription quantities to the workspace billing RPC", async () => {
    const updates: Array<Record<string, unknown>> = [];
    const subscription = {
      customer: "cus_team123",
      items: {
        data: [{ price: { id: "price_team" }, quantity: 4 }],
      },
    } as Stripe.Subscription;

    await syncBillingBridge(
      event("customer.subscription.updated", subscription),
      dependencies({
        syncWorkspaceCustomer: async (update) => {
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
    const subscription = {
      customer: "cus_team123",
      items: {
        data: [{ price: { id: "price_team" }, quantity: 4 }],
      },
    } as Stripe.Subscription;

    await expect(
      syncBillingBridge(
        event("customer.subscription.updated", subscription),
        dependencies({
          syncWorkspaceCustomer: async () => "cus_another_workspace",
        }),
      ),
    ).rejects.toThrow("Workspace Stripe customer assignment conflict");
  });

  it("ignores Stripe events for a workspace that no longer exists", async () => {
    const subscription = {
      customer: "cus_team123",
      items: {
        data: [{ price: { id: "price_team" }, quantity: 4 }],
      },
    } as Stripe.Subscription;

    await expect(
      syncBillingBridge(
        event("customer.subscription.updated", subscription),
        dependencies({
          syncWorkspaceCustomer: async () => null,
        }),
      ),
    ).resolves.toBeUndefined();
  });

  it("preserves personal customer assignment and metadata repair", async () => {
    const metadataUpdates: Array<Record<string, string>> = [];
    const assignments: string[][] = [];

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
});
