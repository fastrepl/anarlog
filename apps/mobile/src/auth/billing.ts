// Ported from packages/supabase/src/jwt.ts + billing.ts — @anlg/supabase pulls
// in jose, which does not run on Hermes. Keep semantics in sync.

export type SubscriptionStatus =
  | "incomplete"
  | "incomplete_expired"
  | "trialing"
  | "active"
  | "past_due"
  | "canceled"
  | "unpaid"
  | "paused";

export type SupabaseJwtPayload = {
  sub?: string;
  email?: string;
  entitlements?: string[];
  subscription_status?: SubscriptionStatus | null;
  trial_end?: number | null;
  has_payment_method?: boolean | null;
  cancel_at_period_end?: boolean | null;
  current_period_end?: number | null;
};

export type Plan = "free" | "trial" | "pro";

export class ProRequiredError extends Error {
  constructor() {
    super(
      "Anarlog models require an active Pro trial or subscription. Choose a provider with your own API key in Settings to continue.",
    );
  }
}

export type BillingInfo = {
  entitlements: string[];
  subscriptionStatus: SubscriptionStatus | null;
  isPro: boolean;
  isLite: boolean;
  isPaid: boolean;
  isTrialing: boolean;
  isPaused: boolean;
  trialEnd: Date | null;
  trialDaysRemaining: number | null;
  cancelAtPeriodEnd: boolean;
  currentPeriodEnd: Date | null;
  plan: Plan;
};

// Unverified decode — display/gating only, never security.
export function decodeJwtPayload(
  accessToken: string,
): SupabaseJwtPayload | null {
  try {
    const segment = accessToken.split(".")[1];
    if (!segment) {
      return null;
    }

    const base64 = segment.replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64.padEnd(
      base64.length + ((4 - (base64.length % 4)) % 4),
      "=",
    );
    const binary = atob(padded);

    let percentEncoded = "";
    for (let i = 0; i < binary.length; i++) {
      percentEncoded +=
        "%" + binary.charCodeAt(i).toString(16).padStart(2, "0");
    }
    const json = decodeURIComponent(percentEncoded);

    const parsed = JSON.parse(json);
    if (typeof parsed !== "object" || parsed === null) {
      return null;
    }
    return parsed as SupabaseJwtPayload;
  } catch {
    return null;
  }
}

export function deriveBillingInfo(
  payload: SupabaseJwtPayload | null,
  now = Date.now(),
): BillingInfo {
  const entitlements = payload?.entitlements ?? [];
  const subscriptionStatus = payload?.subscription_status ?? null;

  const trialEnd = payload?.trial_end
    ? new Date(payload.trial_end * 1000)
    : null;

  let trialDaysRemaining: number | null = null;
  if (trialEnd) {
    const secondsRemaining = (trialEnd.getTime() - now) / 1000;
    trialDaysRemaining =
      secondsRemaining <= 0 ? 0 : Math.ceil(secondsRemaining / (24 * 60 * 60));
  }

  const isTrialing =
    subscriptionStatus === "trialing" &&
    trialDaysRemaining !== null &&
    trialDaysRemaining > 0;

  const hasProEntitlement = entitlements.includes("hyprnote_pro");
  const hasLiteEntitlement = entitlements.includes("hyprnote_lite");
  const isPaused = subscriptionStatus === "paused";
  // Trial clocks and paused subscriptions override stale entitlements so every
  // client fails closed consistently.
  const hasEffectiveProEntitlement =
    subscriptionStatus === "trialing"
      ? isTrialing
      : isPaused
        ? false
        : hasProEntitlement;
  const hasPaidEntitlement = hasEffectiveProEntitlement || hasLiteEntitlement;

  const plan: Plan = isTrialing ? "trial" : hasPaidEntitlement ? "pro" : "free";
  const currentPeriodEnd = payload?.current_period_end
    ? new Date(payload.current_period_end * 1000)
    : null;

  return {
    entitlements,
    subscriptionStatus,
    isPro: hasEffectiveProEntitlement,
    isLite: hasLiteEntitlement,
    isPaid: hasPaidEntitlement,
    isTrialing,
    isPaused,
    trialEnd,
    trialDaysRemaining,
    cancelAtPeriodEnd: payload?.cancel_at_period_end === true,
    currentPeriodEnd,
    plan,
  };
}
