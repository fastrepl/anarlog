import type { SubscriptionStatus, SupabaseJwtPayload } from "./jwt";

export type Plan = "free" | "trial" | "pro";

export type BillingInfo = {
  entitlements: string[];
  subscriptionStatus: SubscriptionStatus | null;
  isPro: boolean;
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
  const isTrialing = subscriptionStatus === "trialing";
  const isPro =
    entitlements.includes("hyprnote_pro") ||
    isTrialing ||
    subscriptionStatus === "active";
  const trialEnd = payload?.trial_end
    ? new Date(payload.trial_end * 1000)
    : null;

  let trialDaysRemaining: number | null = null;
  if (trialEnd) {
    const secondsRemaining = (trialEnd.getTime() - Date.now()) / 1000;
    trialDaysRemaining =
      secondsRemaining <= 0 ? 0 : Math.ceil(secondsRemaining / (24 * 60 * 60));
  }

  const plan: Plan =
    isPro && !isTrialing ? "pro" : isTrialing ? "trial" : "free";

  return {
    entitlements,
    subscriptionStatus,
    isPro,
    isTrialing,
    trialEnd,
    trialDaysRemaining,
    plan,
  };
}
