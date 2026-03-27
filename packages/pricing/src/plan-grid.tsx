import { CheckCircle2, XCircle } from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

import {
  getActionForTier,
  PLAN_TIERS,
  type PlanTier,
  type PlanTierData,
  type TierAction,
} from "./tiers";

export function PlanGrid({
  currentPlan,
  isTrialing,
  trialDaysRemaining,
  canStartTrial,
  isPaid,
  renderAction,
  renderManageBilling,
}: {
  currentPlan: PlanTier;
  isTrialing: boolean;
  trialDaysRemaining: number | null;
  canStartTrial: boolean;
  isPaid: boolean;
  renderAction: (tier: PlanTierData, action: TierAction) => ReactNode;
  renderManageBilling?: () => ReactNode;
}) {
  const statusText = isTrialing
    ? `Pro trial${trialDaysRemaining != null ? ` \u2014 ${trialDaysRemaining} day${trialDaysRemaining === 1 ? "" : "s"} left` : ""}`
    : `You\u2019re on the ${currentPlan === "free" ? "Free" : currentPlan === "lite" ? "Lite" : "Pro"} plan`;

  return (
    <div className="rounded-xs border border-neutral-100">
      <div className="flex items-center justify-between p-4">
        <div>
          <h3 className="mb-2 font-serif text-lg font-semibold">
            Plan & Billing
          </h3>
          <p className="text-sm text-neutral-600">{statusText}</p>
        </div>
        {isPaid && renderManageBilling?.()}
      </div>

      <div className="grid grid-cols-3 gap-px border-t border-neutral-100 bg-neutral-100">
        {PLAN_TIERS.map((tier) => {
          const isCurrent = tier.id === currentPlan;
          const action = getActionForTier(tier.id, currentPlan, canStartTrial);

          return (
            <div
              key={tier.id}
              className={cn([
                "flex flex-col bg-white p-4",
                isCurrent && "bg-stone-50/60",
              ])}
            >
              <div className="mb-3 flex items-center gap-2">
                <span className="font-serif text-base font-medium text-stone-800">
                  {tier.name}
                </span>
                {isCurrent && (
                  <span className="rounded-full bg-stone-600 px-2 py-0.5 text-[10px] font-medium tracking-wide text-white uppercase">
                    {isTrialing ? "Trial" : "Current"}
                  </span>
                )}
              </div>

              <div className="mb-3">
                <span className="font-serif text-2xl text-stone-700">
                  {tier.price}
                </span>
                {tier.period && (
                  <span className="text-sm text-neutral-500">
                    {tier.period}
                  </span>
                )}
                {tier.subtitle && (
                  <div className="mt-0.5 text-xs text-neutral-400">
                    {tier.subtitle}
                  </div>
                )}
              </div>

              <div className="mb-4 flex flex-col gap-1.5">
                {tier.features.map((f) => (
                  <div key={f} className="flex items-start gap-1.5">
                    <CheckCircle2 className="mt-0.5 size-3.5 shrink-0 text-green-700" />
                    <span className="text-xs text-neutral-700">{f}</span>
                  </div>
                ))}
                {tier.notIncluded.map((f) => (
                  <div key={f} className="flex items-start gap-1.5">
                    <XCircle className="mt-0.5 size-3.5 shrink-0 text-neutral-300" />
                    <span className="text-xs text-neutral-400">{f}</span>
                  </div>
                ))}
              </div>

              <div className="mt-auto">{renderAction(tier, action)}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
