import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import {
  canStartTrial,
  createPortalSession,
  startTrial,
} from "@/functions/billing";
import { useBilling } from "@/hooks/use-billing";

export function AccountSettingsCard() {
  const billing = useBilling();

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

  const startTrialMutation = useMutation({
    mutationFn: () => startTrial(),
    onSuccess: () => {
      billing.refreshBilling();
      canTrialQuery.refetch();
    },
  });

  const renderPlanButton = () => {
    if (
      !billing.isReady ||
      (billing.plan === "free" && canTrialQuery.isLoading)
    ) {
      return (
        <div className="flex h-8 items-center px-4 text-sm text-neutral-400">
          Loading...
        </div>
      );
    }

    if (billing.plan === "free") {
      if (canTrialQuery.data) {
        return (
          <button
            onClick={() => startTrialMutation.mutate()}
            disabled={startTrialMutation.isPending}
            className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
          >
            {startTrialMutation.isPending ? "Loading..." : "Start Free Trial"}
          </button>
        );
      }

      return (
        <Link
          to="/app/checkout/"
          search={{ period: "monthly" }}
          className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
        >
          Upgrade to Pro
        </Link>
      );
    }

    return (
      <button
        onClick={() => manageBillingMutation.mutate()}
        disabled={manageBillingMutation.isPending}
        className="flex h-8 cursor-pointer items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
      >
        {manageBillingMutation.isPending ? "Loading..." : "Manage Billing"}
      </button>
    );
  };

  const planDisplay = !billing.isReady
    ? "..."
    : billing.plan === "trial"
      ? "Trial"
      : billing.plan === "pro"
        ? "Pro"
        : "Free";

  return (
    <div className="rounded-xs border border-neutral-100">
      <div className="p-4">
        <h3 className="mb-2 font-serif text-lg font-semibold">
          Account Settings
        </h3>
        <p className="text-sm text-neutral-600">
          Manage your account preferences and billing settings
        </p>
      </div>

      <div className="flex items-center justify-between border-t border-neutral-100 p-4">
        <div className="text-sm">
          Current plan: <span className="font-medium">{planDisplay}</span>
        </div>
        {renderPlanButton()}
      </div>
    </div>
  );
}
