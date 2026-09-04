export type StripeCustomerOwnership = "owned" | "claimable" | "unowned";

const UNAVAILABLE_CUSTOMER_MESSAGES = new Set([
  "Stripe customer is unavailable",
  "Stripe customer does not belong to authenticated user",
]);

export function isUnavailableStripeCustomerError(error: unknown) {
  if (typeof error !== "object" || error === null) return false;

  const raw = Reflect.get(error, "raw");
  const rawCode =
    typeof raw === "object" && raw !== null
      ? Reflect.get(raw, "code")
      : undefined;

  return (
    Reflect.get(error, "code") === "resource_missing" ||
    rawCode === "resource_missing" ||
    UNAVAILABLE_CUSTOMER_MESSAGES.has(Reflect.get(error, "message"))
  );
}

export function getStripeCustomerOwnership(
  customer: {
    email?: string | null;
    metadata?: Record<string, string> | null;
  },
  user: { id: string; email?: string | null },
): StripeCustomerOwnership {
  const metadata = customer.metadata ?? {};
  const ownerIds = [
    metadata["userId"],
    metadata["user_id"],
    metadata["userID"],
  ].filter((ownerId): ownerId is string => Boolean(ownerId));

  if (ownerIds.length > 0) {
    return ownerIds.every((ownerId) => ownerId === user.id)
      ? "owned"
      : "unowned";
  }

  const customerEmail = customer.email?.trim().toLowerCase();
  const userEmail = user.email?.trim().toLowerCase();

  return customerEmail && userEmail && customerEmail === userEmail
    ? "claimable"
    : "unowned";
}

export function getStripeCustomerIdentityMetadata(
  metadata: Record<string, string> | null | undefined,
  userId: string,
) {
  if (
    metadata?.["userId"] === userId &&
    metadata["posthog_person_distinct_id"] === userId
  ) {
    return null;
  }

  return {
    userId,
    posthog_person_distinct_id: userId,
  };
}
