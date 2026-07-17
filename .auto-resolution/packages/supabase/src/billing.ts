import type { SubscriptionStatus, SupabaseJwtPayload } from "./jwt";

export type Plan = "free" | "trial" | "pro";

export type BillingInfo = {
  entitlements: string[];
  subscriptionStatus: SubscriptionStatus | null;
  isPro: boolean;
  isLite: boolean;
  isPaid: boolean;
  isTrialing: boolean;
  trialEnd: Date | null;
  trialDaysRemaining: number | null;
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
    (trialDaysRemaining === null || trialDaysRemaining > 0);

  const hasProEntitlement = entitlements.includes("hyprnote_pro");
  const hasLiteEntitlement = entitlements.includes("hyprnote_lite");
  const hasPaidEntitlement = hasProEntitlement || hasLiteEntitlement;

  const isPro = hasPaidEntitlement || isTrialing;
  const isLite = false;
  const isPaid =
    hasPaidEntitlement || isTrialing || subscriptionStatus === "active";

  const plan: Plan = isTrialing
    ? "trial"
    : hasPaidEntitlement || subscriptionStatus === "active"
      ? "pro"
      : "free";

  return {
    entitlements,
    subscriptionStatus,
    isPro,
    isLite,
    isPaid,
    isTrialing,
    trialEnd,
    trialDaysRemaining,
    plan,
  };
}
