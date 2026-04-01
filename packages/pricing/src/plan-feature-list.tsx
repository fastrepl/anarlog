import { CheckCircle2, Construction, XCircle } from "lucide-react";

import { cn } from "@hypr/utils";

import type { PlanFeature } from "./tiers";

export function PlanFeatureList({
  features,
  dense = false,
}: {
  features: PlanFeature[];
  dense?: boolean;
}) {
  return (
    <div
      className={cn([dense ? "flex flex-col gap-1.5" : "flex flex-col gap-3"])}
    >
      {features.map((feature) => {
        const Icon =
          feature.included === true
            ? CheckCircle2
            : feature.included === "partial"
              ? Construction
              : XCircle;
        const hoverTitle =
          feature.included === "partial"
            ? "Currently in development"
            : undefined;

        return (
          <div
            key={feature.label}
            className={cn([
              dense ? "flex items-start gap-1.5" : "flex items-start gap-3",
            ])}
            title={hoverTitle}
          >
            <Icon
              className={cn([
                dense ? "mt-0.5 size-3.5 shrink-0" : "mt-0.5 size-4.5 shrink-0",
                feature.included === true
                  ? "text-green-700"
                  : feature.included === "partial"
                    ? "text-yellow-600"
                    : "text-red-500",
              ])}
            />
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <span
                  className={cn([
                    dense ? "text-xs" : "text-sm",
                    feature.included === false
                      ? "text-neutral-700"
                      : "text-neutral-900",
                  ])}
                >
                  {feature.label}
                </span>
              </div>
              {feature.tooltip && !dense && (
                <div className="mt-0.5 text-xs text-neutral-500 italic">
                  {feature.tooltip}
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
