import { CheckCircle, XCircle } from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

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
        const Icon = feature.included ? CheckCircle : XCircle;
        const iconContainerClassName = cn([
          dense
            ? "flex h-4 shrink-0 items-center"
            : "flex h-5 shrink-0 items-center",
        ]);
        const iconClassName = cn([
          dense ? "size-3.5" : "size-4.5",
          feature.included
            ? "text-emerald-600 dark:text-emerald-400"
            : "text-red-500 dark:text-red-400",
        ]);

        return (
          <div
            key={feature.label}
            className={cn([
              dense ? "flex items-start gap-1.5" : "flex items-start gap-3",
            ])}
          >
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
                    feature.included
                      ? "text-foreground"
                      : "text-muted-foreground",
                  ])}
                >
                  {feature.label}
                </span>
              </div>
              {feature.tooltip && !dense && (
                <div className="text-muted-foreground mt-0.5 text-xs italic">
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
