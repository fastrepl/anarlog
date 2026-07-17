export type StripeCustomerOwnership = "owned" | "claimable" | "unowned";

export function getStripeCustomerOwnership(
  customer: {
    email?: string | null;
    metadata?: Record<string, string> | null;
  },
  user: { id: string; email?: string | null },
): StripeCustomerOwnership {
  const metadata = customer.metadata ?? {};
  const ownerId =
    metadata["userId"] ?? metadata["user_id"] ?? metadata["userID"];

  if (ownerId) {
    return ownerId === user.id ? "owned" : "unowned";
  }

  const customerEmail = customer.email?.trim().toLowerCase();
  const userEmail = user.email?.trim().toLowerCase();

  return customerEmail && userEmail && customerEmail === userEmail
    ? "claimable"
    : "unowned";
}
