export function getSubscriptionAccessEnd(subscription: {
  cancel_at?: number | null;
  current_period_end?: number | null;
  items?: { data?: Array<{ current_period_end?: number | null }> };
}): number | null {
  if (typeof subscription.cancel_at === "number") {
    return subscription.cancel_at;
  }

  const itemPeriodEnds = (subscription.items?.data ?? [])
    .map((item) => item.current_period_end)
    .filter((value): value is number => typeof value === "number");

  if (itemPeriodEnds.length > 0) {
    return Math.max(...itemPeriodEnds);
  }

  return typeof subscription.current_period_end === "number"
    ? subscription.current_period_end
    : null;
}

export function formatAccountPlanDate(date: Date) {
  return date.toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

export function getAccountPlanCopy({
  isTrialing,
  isPaused = false,
  isPaid,
  isLite,
  isPro,
  trialDaysRemaining,
  trialEnd,
  cancelAtPeriodEnd,
  currentPeriodEnd,
  hasYcPerk = false,
}: {
  isTrialing: boolean;
  isPaused?: boolean;
  isPaid: boolean;
  isLite?: boolean;
  isPro?: boolean;
  trialDaysRemaining: number | null;
  trialEnd: Date | null;
  cancelAtPeriodEnd: boolean;
  currentPeriodEnd: Date | null;
  hasYcPerk?: boolean;
}): { planLabel: string; planDetail: string } {
  const planLabel = isTrialing
    ? "Pro trial"
    : isPaid
      ? isLite && !isPro
        ? "Lite"
        : "Pro"
      : "Free";

  if (isTrialing) {
    return {
      planLabel,
      planDetail: trialEnd
        ? `${trialDaysRemaining} ${
            trialDaysRemaining === 1 ? "day" : "days"
          } left · ends ${trialEnd.toLocaleDateString("en-US", {
            month: "long",
            day: "numeric",
          })}.`
        : "Your trial is running.",
    };
  }

  if (isPaused) {
    return {
      planLabel,
      planDetail: "Your Pro trial ended. Resume it to reactivate Pro.",
    };
  }

  if (!isPaid) {
    return {
      planLabel,
      planDetail: "On-device basics, free forever.",
    };
  }

  if (cancelAtPeriodEnd) {
    return {
      planLabel,
      planDetail: currentPeriodEnd
        ? `Cancels ${formatAccountPlanDate(currentPeriodEnd)}.`
        : "Cancels at the end of the billing period.",
    };
  }

  if (hasYcPerk) {
    return {
      planLabel,
      planDetail: "YC founder year is applied.",
    };
  }

  return {
    planLabel,
    planDetail: "Thanks for supporting Anarlog.",
  };
}
