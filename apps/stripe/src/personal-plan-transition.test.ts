import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { scheduleReplacedPersonalPlanCancellation } from "./personal-plan-transition";

const workspaceId = "workspace-123";
const personalUserId = "user-123";

const subscription = ({
  id = "sub_team123",
  customer = "cus_team123",
  status = "active",
  cancelAtPeriodEnd = false,
  metadata = {
    checkout_type: "team",
    workspace_id: workspaceId,
    replaces_personal_subscription_id: "sub_personal123",
    replaces_personal_user_id: personalUserId,
  },
}: {
  id?: string;
  customer?: string;
  status?: Stripe.Subscription.Status;
  cancelAtPeriodEnd?: boolean;
  metadata?: Record<string, string>;
} = {}) =>
  ({
    id,
    customer,
    status,
    cancel_at_period_end: cancelAtPeriodEnd,
    metadata,
  }) as Stripe.Subscription;

const event = (
  value: Stripe.Subscription,
  type: Stripe.Event.Type = "customer.subscription.created",
) =>
  ({
    id: "evt_team123",
    type,
    data: { object: value },
  }) as Stripe.Event;

function dependencies(
  personal: Stripe.Subscription = subscription({
    id: "sub_personal123",
    customer: "cus_personal123",
  }),
) {
  const scheduled: string[] = [];
  return {
    scheduled,
    value: {
      getCustomer: async (customerId: string) =>
        ({
          id: customerId,
          metadata:
            customerId === "cus_team123"
              ? { workspaceId }
              : { userId: personalUserId },
        }) as unknown as Stripe.Customer,
      getSubscription: async () => personal,
      scheduleCancellation: async (subscriptionId: string) => {
        scheduled.push(subscriptionId);
      },
    },
  };
}

describe("scheduleReplacedPersonalPlanCancellation", () => {
  it("schedules the replaced personal plan to end after Team activates", async () => {
    const deps = dependencies();

    await scheduleReplacedPersonalPlanCancellation(
      event(subscription()),
      deps.value,
    );

    expect(deps.scheduled).toEqual(["sub_personal123"]);
  });

  for (const status of [
    "past_due",
    "unpaid",
  ] satisfies Stripe.Subscription.Status[]) {
    it(`schedules a ${status} personal plan that Stripe may still retry`, async () => {
      const deps = dependencies(
        subscription({
          id: "sub_personal123",
          customer: "cus_personal123",
          status,
        }),
      );

      await scheduleReplacedPersonalPlanCancellation(
        event(subscription()),
        deps.value,
      );

      expect(deps.scheduled).toEqual(["sub_personal123"]);
    });
  }

  it("waits for the Team subscription to become active", async () => {
    const deps = dependencies();

    await scheduleReplacedPersonalPlanCancellation(
      event(subscription({ status: "incomplete" })),
      deps.value,
    );

    expect(deps.scheduled).toEqual([]);
  });

  it("ignores Team subscriptions created before replacement metadata existed", async () => {
    const deps = dependencies();

    await scheduleReplacedPersonalPlanCancellation(
      event(
        subscription({
          metadata: {
            checkout_type: "team",
            workspace_id: workspaceId,
          },
        }),
      ),
      deps.value,
    );

    expect(deps.scheduled).toEqual([]);
  });

  it("is idempotent when the personal plan already ends after its period", async () => {
    const deps = dependencies(
      subscription({
        id: "sub_personal123",
        customer: "cus_personal123",
        cancelAtPeriodEnd: true,
      }),
    );

    await scheduleReplacedPersonalPlanCancellation(
      event(subscription()),
      deps.value,
    );

    expect(deps.scheduled).toEqual([]);
  });

  it("does not cancel a paused personal plan", async () => {
    const deps = dependencies(
      subscription({
        id: "sub_personal123",
        customer: "cus_personal123",
        status: "paused",
      }),
    );

    await scheduleReplacedPersonalPlanCancellation(
      event(subscription()),
      deps.value,
    );

    expect(deps.scheduled).toEqual([]);
  });

  it("fails closed when the Team customer does not own the workspace", async () => {
    const deps = dependencies();
    deps.value.getCustomer = async (customerId) =>
      ({
        id: customerId,
        metadata:
          customerId === "cus_team123"
            ? { workspaceId: "another-workspace" }
            : { userId: personalUserId },
      }) as unknown as Stripe.Customer;

    await expect(
      scheduleReplacedPersonalPlanCancellation(
        event(subscription()),
        deps.value,
      ),
    ).rejects.toThrow("Team subscription customer ownership is invalid");
    expect(deps.scheduled).toEqual([]);
  });

  it("fails closed when the personal plan belongs to another user", async () => {
    const deps = dependencies();
    deps.value.getCustomer = async (customerId) =>
      ({
        id: customerId,
        metadata:
          customerId === "cus_team123"
            ? { workspaceId }
            : { userId: "another-user" },
      }) as unknown as Stripe.Customer;

    await expect(
      scheduleReplacedPersonalPlanCancellation(
        event(subscription()),
        deps.value,
      ),
    ).rejects.toThrow("Personal subscription customer ownership is invalid");
    expect(deps.scheduled).toEqual([]);
  });
});
