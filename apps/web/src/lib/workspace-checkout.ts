export type WorkspaceCheckoutContext = {
  workspaceName: string;
  stripeCustomerId: string | null;
  usedSeats: number;
};

export type PersonalPlanReplacement = {
  subscriptionId: string;
  userId: string;
};

type WorkspaceCustomer = {
  id: string;
  deleted?: boolean;
  metadata?: Record<string, string> | null;
};

type WorkspaceSubscription = {
  status: string;
};

type WorkspaceCheckoutDependencies = {
  getContext: (workspaceId: string) => Promise<WorkspaceCheckoutContext>;
  getPriceId: (period: "monthly" | "yearly") => string;
  createCustomer: (input: {
    workspaceId: string;
    workspaceName: string;
  }) => Promise<{ id: string }>;
  bindCustomer: (
    workspaceId: string,
    customerId: string,
  ) => Promise<string | null | undefined>;
  deleteCustomer: (customerId: string) => Promise<void>;
  retrieveCustomer: (customerId: string) => Promise<WorkspaceCustomer>;
  listSubscriptions: (customerId: string) => Promise<WorkspaceSubscription[]>;
  createCheckoutSession: (input: {
    customerId: string;
    priceId: string;
    quantity: number;
    minimumQuantity: number;
    workspaceId: string;
    personalPlanReplacement?: PersonalPlanReplacement;
    successUrl: string;
    cancelUrl: string;
  }) => Promise<{ url: string | null }>;
  createPortalSession: (input: {
    customerId: string;
    returnUrl: string;
  }) => Promise<{ url: string }>;
};

const MANAGEABLE_SUBSCRIPTION_STATUSES = new Set([
  "active",
  "trialing",
  "past_due",
  "unpaid",
  "incomplete",
  "paused",
]);

export async function startWorkspaceCheckout(
  input: {
    workspaceId: string;
    period: "monthly" | "yearly";
    quantity: number;
    successUrl: string;
    cancelUrl: string;
    returnUrl: string;
    personalPlanReplacement?: PersonalPlanReplacement;
  },
  dependencies: WorkspaceCheckoutDependencies,
) {
  const context = await dependencies.getContext(input.workspaceId);

  if (context.usedSeats < 1 || input.quantity < context.usedSeats) {
    throw new Error("Team checkout must cover every occupied seat");
  }

  let priceId: string | undefined;
  let customerId = context.stripeCustomerId;

  if (!customerId) {
    priceId = dependencies.getPriceId(input.period);
    const createdCustomer = await dependencies.createCustomer({
      workspaceId: input.workspaceId,
      workspaceName: context.workspaceName,
    });

    let assignedCustomerId: string | null | undefined;
    try {
      assignedCustomerId = await dependencies.bindCustomer(
        input.workspaceId,
        createdCustomer.id,
      );
    } catch (error) {
      await dependencies
        .deleteCustomer(createdCustomer.id)
        .catch(() => undefined);
      throw error;
    }

    if (!assignedCustomerId) {
      await dependencies
        .deleteCustomer(createdCustomer.id)
        .catch(() => undefined);
      throw new Error("Workspace billing is unavailable");
    }

    if (assignedCustomerId !== createdCustomer.id) {
      await dependencies
        .deleteCustomer(createdCustomer.id)
        .catch(() => undefined);
    }
    customerId = assignedCustomerId;
  }

  const customer = await dependencies.retrieveCustomer(customerId);
  if (
    customer.deleted ||
    getWorkspaceCustomerOwnership(customer.metadata, input.workspaceId) !==
      "owned"
  ) {
    throw new Error("Stripe customer does not belong to this workspace");
  }

  const subscriptions = await dependencies.listSubscriptions(customerId);
  if (
    subscriptions.some((subscription) =>
      MANAGEABLE_SUBSCRIPTION_STATUSES.has(subscription.status),
    )
  ) {
    const portal = await dependencies.createPortalSession({
      customerId,
      returnUrl: input.returnUrl,
    });
    return { url: portal.url, stripeCustomerId: customerId };
  }

  priceId ??= dependencies.getPriceId(input.period);
  const checkout = await dependencies.createCheckoutSession({
    customerId,
    priceId,
    quantity: input.quantity,
    minimumQuantity: context.usedSeats,
    workspaceId: input.workspaceId,
    personalPlanReplacement: input.personalPlanReplacement,
    successUrl: input.successUrl,
    cancelUrl: input.cancelUrl,
  });
  return { url: checkout.url, stripeCustomerId: customerId };
}

export function getWorkspaceCustomerOwnership(
  metadata: Record<string, string> | null | undefined,
  workspaceId: string,
) {
  const workspaceIds = [
    metadata?.["workspaceId"],
    metadata?.["workspace_id"],
  ].filter((value): value is string => Boolean(value));
  const userIds = [
    metadata?.["userId"],
    metadata?.["user_id"],
    metadata?.["userID"],
  ].filter((value): value is string => Boolean(value));

  return userIds.length === 0 &&
    workspaceIds.length > 0 &&
    workspaceIds.every((value) => value === workspaceId)
    ? "owned"
    : "unowned";
}
