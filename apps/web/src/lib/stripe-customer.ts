export type StripeCustomerOwnership = "owned" | "claimable" | "unowned";

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
