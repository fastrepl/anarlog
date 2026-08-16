import assert from "node:assert/strict";
import test from "node:test";

import {
  getWorkspaceCustomerOwnership,
  startWorkspaceCheckout,
  type WorkspaceCheckoutContext,
} from "./workspace-checkout.ts";

const workspaceId = "0d69f853-2010-45e7-83ab-a7f14375f506";

function dependencies(
  context: WorkspaceCheckoutContext = {
    workspaceName: "Checkout HQ",
    stripeCustomerId: "cus_team123",
    usedSeats: 2,
  },
) {
  const checkoutInputs: unknown[] = [];
  const portalInputs: unknown[] = [];
  const deletedCustomers: string[] = [];
  return {
    checkoutInputs,
    portalInputs,
    deletedCustomers,
    value: {
      getContext: async () => context,
      getPriceId: (period: "monthly" | "yearly") => `price_pro_${period}`,
      createCustomer: async () => ({ id: "cus_created123" }),
      bindCustomer: async (_workspaceId: string, customerId: string) =>
        customerId,
      deleteCustomer: async (customerId: string) => {
        deletedCustomers.push(customerId);
      },
      retrieveCustomer: async (customerId: string) => ({
        id: customerId,
        metadata: { workspaceId },
      }),
      listSubscriptions: async (): Promise<{ status: string }[]> => [],
      createCheckoutSession: async (input: unknown) => {
        checkoutInputs.push(input);
        return { url: "https://checkout.stripe.test/team" };
      },
      createPortalSession: async (input: unknown) => {
        portalInputs.push(input);
        return { url: "https://billing.stripe.test/team" };
      },
    },
  };
}

const input = {
  workspaceId,
  period: "monthly" as const,
  quantity: 3,
  successUrl: "https://anarlog.test/success",
  cancelUrl: "https://anarlog.test/cancel",
  returnUrl: "https://anarlog.test/team",
};

test("provisions a workspace customer and starts per-seat checkout", async () => {
  const deps = dependencies({
    workspaceName: "Checkout HQ",
    stripeCustomerId: null,
    usedSeats: 2,
  });

  const result = await startWorkspaceCheckout(input, deps.value);

  assert.deepEqual(result, {
    url: "https://checkout.stripe.test/team",
    stripeCustomerId: "cus_created123",
  });
  assert.deepEqual(deps.checkoutInputs, [
    {
      customerId: "cus_created123",
      priceId: "price_pro_monthly",
      quantity: 3,
      minimumQuantity: 2,
      workspaceId,
      successUrl: input.successUrl,
      cancelUrl: input.cancelUrl,
    },
  ]);
});

test("rejects checkout that would underfill occupied seats", async () => {
  const deps = dependencies();

  await assert.rejects(
    startWorkspaceCheckout({ ...input, quantity: 1 }, deps.value),
    /cover every occupied seat/,
  );
  assert.deepEqual(deps.checkoutInputs, []);
});

test("a concurrent customer assignment wins without leaking the loser", async () => {
  const deps = dependencies({
    workspaceName: "Checkout HQ",
    stripeCustomerId: null,
    usedSeats: 2,
  });
  deps.value.bindCustomer = async () => "cus_existing123";

  const result = await startWorkspaceCheckout(input, deps.value);

  assert.equal(result.stripeCustomerId, "cus_existing123");
  assert.deepEqual(deps.deletedCustomers, ["cus_created123"]);
});

test("missing Pro pricing fails before provisioning a customer", async () => {
  const deps = dependencies({
    workspaceName: "Checkout HQ",
    stripeCustomerId: null,
    usedSeats: 2,
  });
  let created = false;
  deps.value.getPriceId = () => {
    throw new Error("Missing Pro price");
  };
  deps.value.createCustomer = async () => {
    created = true;
    return { id: "cus_created123" };
  };

  await assert.rejects(
    startWorkspaceCheckout(input, deps.value),
    /Missing Pro price/,
  );
  assert.equal(created, false);
});

for (const status of ["active", "trialing", "past_due", "unpaid"]) {
  test(`${status} subscriptions reopen the portal instead of duplicating`, async () => {
    const deps = dependencies();
    deps.value.listSubscriptions = async () => [{ status }];

    const result = await startWorkspaceCheckout(input, deps.value);

    assert.equal(result.url, "https://billing.stripe.test/team");
    assert.deepEqual(deps.checkoutInputs, []);
    assert.deepEqual(deps.portalInputs, [
      {
        customerId: "cus_team123",
        returnUrl: input.returnUrl,
      },
    ]);
  });
}

test("an existing subscription can reach the portal without Pro price config", async () => {
  const deps = dependencies();
  deps.value.getPriceId = () => {
    throw new Error("Missing Pro price");
  };
  deps.value.listSubscriptions = async () => [{ status: "active" }];

  const result = await startWorkspaceCheckout(input, deps.value);

  assert.equal(result.url, "https://billing.stripe.test/team");
});

test("workspace metadata cannot also claim a personal owner", () => {
  assert.equal(
    getWorkspaceCustomerOwnership({ workspace_id: workspaceId }, workspaceId),
    "owned",
  );
  assert.equal(
    getWorkspaceCustomerOwnership(
      { workspaceId, userId: "other-user" },
      workspaceId,
    ),
    "unowned",
  );
  assert.equal(
    getWorkspaceCustomerOwnership(
      { workspaceId: "other-workspace" },
      workspaceId,
    ),
    "unowned",
  );
});
