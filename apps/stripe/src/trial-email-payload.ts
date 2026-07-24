import type Stripe from "stripe";

export function buildTrialEndingEmail({
  subscription,
  customer,
  now,
}: {
  subscription: Stripe.Subscription;
  customer: Stripe.Customer;
  now: number;
}): { email: string; dataVariables: Record<string, string | number> } | null {
  if (!customer.email || !subscription.trial_end) {
    return null;
  }

  // Same payment-method presence rule as the has_payment_method auth claim:
  // card-backed trials auto-convert, so they need no reminder.
  const hasPaymentMethod =
    subscription.default_payment_method != null ||
    customer.invoice_settings?.default_payment_method != null ||
    customer.default_source != null;
  if (hasPaymentMethod) {
    return null;
  }

  const trialEndMs = subscription.trial_end * 1000;
  if (trialEndMs <= now) {
    return null;
  }

  return {
    email: customer.email,
    dataVariables: {
      firstName: customer.name?.trim().split(/\s+/)[0] || "there",
    },
  };
}
