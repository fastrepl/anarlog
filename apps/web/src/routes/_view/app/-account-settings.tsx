import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import type { PlanTierData, TierAction } from "@hypr/pricing";
import { PlanGrid } from "@hypr/pricing/ui";
import { cn } from "@hypr/utils";

import {
  canStartTrial,
  createPlanSwitchSession,
  createPortalSession,
  startTrial,
} from "@/functions/billing";
import { useBilling } from "@/hooks/use-billing";

export function AccountSettingsCard() {
  const billing = useBilling();

  const currentTier = billing.plan === "trial" ? "pro" : billing.plan;

  const canTrialQuery = useQuery({
    queryKey: ["canStartTrial"],
    queryFn: () => canStartTrial(),
    enabled: billing.isReady && billing.plan === "free",
  });

  const manageBillingMutation = useMutation({
    mutationFn: async () => {
      const { url } = await createPortalSession();
      if (url) {
        window.location.href = url;
      }
    },
  });

  const switchPlanMutation = useMutation({
    mutationFn: async (targetPlan: "lite" | "pro") => {
      const { url } = await createPlanSwitchSession({
        data: { targetPlan, targetPeriod: "monthly" },
      });
      if (url) {
        window.location.href = url;
      }
    },
  });

  const startTrialMutation = useMutation({
    mutationFn: () => startTrial(),
    onSuccess: () => {
      billing.refreshBilling();
      canTrialQuery.refetch();
    },
  });

  const isLoading =
    !billing.isReady || (billing.plan === "free" && canTrialQuery.isLoading);

  if (isLoading) {
    return (
      <div className="rounded-xs border border-neutral-100 p-4">
        <h3 className="mb-2 font-serif text-lg font-semibold">
          Plan & Billing
        </h3>
        <p className="text-sm text-neutral-400">Loading...</p>
      </div>
    );
  }

  return (
    <PlanGrid
      currentPlan={currentTier}
      isTrialing={billing.isTrialing}
      trialDaysRemaining={billing.trialDaysRemaining}
      canStartTrial={canTrialQuery.data ?? false}
      isPaid={billing.isPaid}
      renderManageBilling={() => (
        <button
          onClick={() => manageBillingMutation.mutate()}
          disabled={manageBillingMutation.isPending}
          className="flex h-8 cursor-pointer items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
        >
          {manageBillingMutation.isPending ? "Loading..." : "Manage billing"}
        </button>
      )}
      renderAction={(_tier: PlanTierData, action: TierAction) => {
        if (action == null) return null;

        if (action.style === "current") {
          return (
            <div className="flex h-8 items-center justify-center rounded-full border border-neutral-200 bg-neutral-50 text-xs text-neutral-500">
              {action.label}
            </div>
          );
        }

        const buttonClass = cn([
          "flex h-8 w-full items-center justify-center rounded-full text-xs font-medium transition-all hover:scale-[102%] active:scale-[98%]",
          action.style === "upgrade"
            ? "bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:shadow-lg"
            : "border border-neutral-300 bg-linear-to-b from-white to-stone-50 text-neutral-700 shadow-xs hover:shadow-md",
        ]);

        if (action.targetPlan && currentTier === "free") {
          if (action.label === "Start free trial") {
            return (
              <button
                onClick={() => startTrialMutation.mutate()}
                disabled={startTrialMutation.isPending}
                className={cn([
                  buttonClass,
                  "cursor-pointer disabled:opacity-50 disabled:hover:scale-100",
                ])}
              >
                {startTrialMutation.isPending ? "Loading..." : action.label}
              </button>
            );
          }
          return (
            <Link
              to="/app/checkout/"
              search={{ plan: action.targetPlan, period: "monthly" }}
              className={buttonClass}
            >
              {action.label}
            </Link>
          );
        }

        return (
          <button
            onClick={() => {
              if (action.targetPlan) {
                switchPlanMutation.mutate(action.targetPlan);
              }
            }}
            disabled={switchPlanMutation.isPending}
            className={cn([
              buttonClass,
              "cursor-pointer disabled:opacity-50 disabled:hover:scale-100",
            ])}
          >
            {switchPlanMutation.isPending ? "Loading..." : action.label}
          </button>
        );
      }}
    />
  );
}
