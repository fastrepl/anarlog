import { CheckCircle2, Construction, XCircle } from "lucide-react";

import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import type { PlanFeature } from "./tiers";

export function PlanFeatureList({
  features,
  dense = false,
}: {
  features: PlanFeature[];
  dense?: boolean;
}) {
  const getPartialFeatureTooltip = (feature: PlanFeature) =>
    feature.tooltip
      ? `Currently in development. ${feature.tooltip}`
      : "Currently in development";

  return (
    <TooltipProvider delayDuration={100}>
      <div
        className={cn([
          dense ? "flex flex-col gap-1.5" : "flex flex-col gap-3",
        ])}
      >
        {features.map((feature) => {
          const Icon =
            feature.included === true
              ? CheckCircle2
              : feature.included === "partial"
                ? Construction
                : XCircle;
          const isPartial = feature.included === "partial";
          const iconContainerClassName = cn([
            dense
              ? "flex h-4 shrink-0 items-center"
              : "flex h-5 shrink-0 items-center",
          ]);
          const iconClassName = cn([
            dense ? "size-3.5" : "size-4.5",
            feature.included === true
              ? "text-green-700"
              : isPartial
                ? "text-neutral-900"
                : "text-red-500",
          ]);
          const featureContent = (
            <>
              <div className={iconContainerClassName}>
                <Icon className={iconClassName} />
              </div>
              <div className="flex-1">
                <div
                  className={cn([
                    dense
                      ? "flex min-h-4 items-center gap-2"
                      : "flex min-h-5 items-center gap-2",
                  ])}
                >
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
            </>
          );

          return (
            <div
              key={feature.label}
              className={cn([
                dense ? "flex items-start gap-1.5" : "flex items-start gap-3",
              ])}
            >
              {isPartial ? (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <button
                      type="button"
                      className={cn([
                        "flex w-full items-start border-0 bg-transparent p-0 text-left",
                        dense ? "gap-1.5" : "gap-3",
                        "cursor-help rounded-sm focus-visible:ring-2 focus-visible:ring-stone-300 focus-visible:outline-none",
                      ])}
                      aria-label={`${feature.label}: ${getPartialFeatureTooltip(feature)}`}
                    >
                      {featureContent}
                    </button>
                  </TooltipTrigger>
                  <TooltipContent className="max-w-56">
                    {getPartialFeatureTooltip(feature)}
                  </TooltipContent>
                </Tooltip>
              ) : (
                featureContent
              )}
            </div>
          );
        })}
      </div>
    </TooltipProvider>
  );
}
