import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import { scheduleReplacedPersonalPlanCancellation } from "./personal-plan-transition";

const workspaceId = "workspace-123";
const personalUserId = "user-123";

const personalPlanCases: Array<{
  status: Stripe.Subscription.Status;
  cancelAtPeriodEnd?: boolean;
  expected: string[];
}> = [
  { status: "active", expected: ["sub_personal123"] },
  { status: "past_due", expected: ["sub_personal123"] },
  { status: "unpaid", expected: ["sub_personal123"] },
  { status: "paused", expected: [] },
  { status: "active", cancelAtPeriodEnd: true, expected: [] },
];

const ownershipCases: Array<[string, string, string]> = [
  [
    "another-workspace",
    personalUserId,
    "Team subscription customer ownership is invalid",
  ],
  [
    workspaceId,
    "another-user",
    "Personal subscription customer ownership is invalid",
  ],
];

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
  it.each(personalPlanCases)(
    "handles a replaced personal plan in the $status state",
    async ({ status, cancelAtPeriodEnd, expected }) => {
      const deps = dependencies(
        subscription({
          id: "sub_personal123",
          customer: "cus_personal123",
          status,
          cancelAtPeriodEnd,
        }),
      );

      await scheduleReplacedPersonalPlanCancellation(
        event(subscription()),
        deps.value,
      );

      expect(deps.scheduled).toEqual(expected);
    },
  );

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

  it.each(ownershipCases)(
    "fails closed for conflicting customer ownership",
    async (teamWorkspaceId, personalCustomerUserId, expectedError) => {
      const deps = dependencies();
      deps.value.getCustomer = async (customerId) =>
        ({
          id: customerId,
          metadata:
            customerId === "cus_team123"
              ? { workspaceId: teamWorkspaceId }
              : { userId: personalCustomerUserId },
        }) as unknown as Stripe.Customer;

      await expect(
        scheduleReplacedPersonalPlanCancellation(
          event(subscription()),
          deps.value,
        ),
      ).rejects.toThrow(expectedError);
      expect(deps.scheduled).toEqual([]);
    },
  );
});
