import type { SubscriptionStatus, SupabaseJwtPayload } from "./jwt";

export type Plan = "free" | "trial" | "pro";

export type BillingInfo = {
  entitlements: string[];
  subscriptionStatus: SubscriptionStatus | null;
  isPro: boolean;
  isLite: boolean;
  isPaid: boolean;
  isTrialing: boolean;
  hasPaymentMethod: boolean;
  trialEnd: Date | null;
  trialDaysRemaining: number | null;
  cancelAtPeriodEnd: boolean;
  currentPeriodEnd: Date | null;
  plan: Plan;
};

export function deriveBillingInfo(
  payload: SupabaseJwtPayload | null,
): BillingInfo {
  const entitlements = payload?.entitlements ?? [];
  const subscriptionStatus = payload?.subscription_status ?? null;

  const trialEnd = payload?.trial_end
    ? new Date(payload.trial_end * 1000)
    : null;

  let trialDaysRemaining: number | null = null;
  if (trialEnd) {
    const secondsRemaining = (trialEnd.getTime() - Date.now()) / 1000;
    trialDaysRemaining =
      secondsRemaining <= 0 ? 0 : Math.ceil(secondsRemaining / (24 * 60 * 60));
  }

  const isTrialing =
    subscriptionStatus === "trialing" &&
    trialDaysRemaining !== null &&
    trialDaysRemaining > 0;

  const hasProEntitlement = entitlements.includes("hyprnote_pro");
  const hasLiteEntitlement = entitlements.includes("hyprnote_lite");
  const hasEffectiveProEntitlement =
    subscriptionStatus === "trialing" ? isTrialing : hasProEntitlement;
  const hasPaidEntitlement = hasEffectiveProEntitlement || hasLiteEntitlement;

  const isPro = hasEffectiveProEntitlement;
  const isLite = hasLiteEntitlement;
  const isPaid = hasPaidEntitlement;

  const plan: Plan = isTrialing ? "trial" : hasPaidEntitlement ? "pro" : "free";
  const currentPeriodEnd = payload?.current_period_end
    ? new Date(payload.current_period_end * 1000)
    : null;

  return {
    entitlements,
    subscriptionStatus,
    isPro,
    isLite,
    isPaid,
    isTrialing,
    hasPaymentMethod: payload?.has_payment_method === true,
    trialEnd,
    trialDaysRemaining,
    cancelAtPeriodEnd: payload?.cancel_at_period_end === true,
    currentPeriodEnd,
    plan,
  };
}
