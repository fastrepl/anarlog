import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { getAccountSubscription } from "@/functions/billing";
import { getAccountPlanCopy } from "@/lib/account-plan";

import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillPrimaryClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

export function PlanSection() {
  const { data, isPending } = useAccountSession();
  const billing = data?.billing;
  const subscriptionQuery = useQuery({
    queryKey: ["account-subscription"],
    enabled:
      typeof window !== "undefined" &&
      (billing?.isPaid === true || billing?.isTrialing === true),
    queryFn: () => getAccountSubscription(),
  });

  const cancelAtPeriodEnd =
    subscriptionQuery.data?.cancelAtPeriodEnd ??
    billing?.cancelAtPeriodEnd ??
    false;
  const currentPeriodEnd =
    subscriptionQuery.data?.currentPeriodEnd != null
      ? new Date(subscriptionQuery.data.currentPeriodEnd * 1000)
      : (billing?.currentPeriodEnd ?? null);

  const { planLabel, planDetail } = getAccountPlanCopy({
    isTrialing: billing?.isTrialing === true,
    isPaid: billing?.isPaid === true,
    isLite: billing?.isLite,
    isPro: billing?.isPro,
    trialDaysRemaining: billing?.trialDaysRemaining ?? null,
    trialEnd: billing?.trialEnd ?? null,
    cancelAtPeriodEnd,
    currentPeriodEnd,
  });

  const isCheckingPlan =
    isPending ||
    (billing?.isPaid === true &&
      billing.isTrialing !== true &&
      subscriptionQuery.isPending);

  return (
    <div className={accountCardClassName}>
      <div className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:p-8">
        {isCheckingPlan ? (
          <p className="text-sm leading-6 text-[#756b5d]">
            Checking your plan...
          </p>
        ) : (
          <>
            <div>
              <p className="text-base font-medium text-[#181613]">
                You're on{" "}
                <mark className="bg-[#fff0b3] px-1 font-semibold">
                  {planLabel}
                </mark>
              </p>
              <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                {planDetail}
              </p>
            </div>
            {billing?.isPaid || billing?.isTrialing ? (
              <Link to="/app/portal/" className={accountPillSecondaryClassName}>
                Manage billing
              </Link>
            ) : (
              <Link
                to="/app/checkout/"
                search={{
                  period: "monthly",
                  trial: "false",
                  source: "settings",
                }}
                className={accountPillPrimaryClassName}
              >
                Upgrade to Pro
              </Link>
            )}
          </>
        )}
      </div>
    </div>
  );
}
