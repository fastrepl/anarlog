import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";

import { signOutFn } from "@/functions/auth";
import {
  canStartTrial,
  createPortalSession,
  startTrial,
} from "@/functions/billing";
import { useBilling } from "@/hooks/use-billing";

export function SettingsPanel() {
  const navigate = useNavigate();
  const billing = useBilling();

  const canTrialQuery = useQuery({
    queryKey: ["canStartTrial"],
    queryFn: () => canStartTrial(),
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

  const signOut = useMutation({
    mutationFn: async () => {
      const res = await signOutFn();
      if (res.success) return true;
      throw new Error(res.message);
    },
    onSuccess: () => navigate({ to: "/" }),
    onError: () => navigate({ to: "/" }),
  });

  const planDisplay = !billing.isReady
    ? "..."
    : billing.plan === "trial"
      ? "Trial"
      : billing.plan === "pro"
        ? "Pro"
        : "Free";

  const renderPlanButton = () => {
    if (!billing.isReady || canTrialQuery.isLoading) {
      return <span className="text-sm text-neutral-400">Loading...</span>;
    }

    if (billing.plan === "free") {
      if (canTrialQuery.data) {
        return (
          <button
            onClick={() => startTrialMutation.mutate()}
            disabled={startTrialMutation.isPending}
            className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50"
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
        className="flex h-8 cursor-pointer items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50"
      >
        {manageBillingMutation.isPending ? "Loading..." : "Manage Billing"}
      </button>
    );
  };

  return (
    <div className="px-6 py-8">
      <h1 className="text-xl font-semibold">Settings</h1>

      <div className="mt-6 flex flex-col gap-6">
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

        <section className="pt-2">
          <button
            onClick={() => signOut.mutate()}
            disabled={signOut.isPending}
            className="flex h-8 cursor-pointer items-center rounded-full border border-red-200 px-4 text-sm text-red-600 transition-all hover:border-red-300 hover:text-red-700 disabled:opacity-50"
          >
            {signOut.isPending ? "Signing out..." : "Sign out"}
          </button>
        </section>
      </div>
    </div>
  );
}
