import { Icon } from "@iconify-icon/react";
import { Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export function SolutionHero({
  icon,
  badgeText,
  title,
  description,
  primaryCTA,
  secondaryCTA,
}: {
  icon: string;
  badgeText: string;
  title: string;
  description: string;
  primaryCTA: { label: string; to: string };
  secondaryCTA?: { label: string; to: string };
}) {
  return (
    <div className="pt-12">
      <div className="border-brand-bright flex min-h-[70vh] flex-col justify-between rounded-xl border px-8 py-4 lg:py-8">
        <header className="mb-8 text-left">
          <div className="text-fg mb-3 inline-flex items-center gap-2 py-2 text-sm">
            <Icon icon={icon} className="text-lg" />
            <span>{badgeText}</span>
          </div>
          <h1 className="text-fg mb-6 font-mono text-2xl tracking-tight whitespace-pre-line sm:text-6xl">
            {title}
          </h1>
          <p className="text-fg max-w-2xl text-lg sm:text-xl">{description}</p>
          <div className="mt-8 flex flex-col justify-start gap-4 sm:flex-row">
            <Link
              to={primaryCTA.to}
              className={cn([
                "inline-block rounded-full px-8 py-3 text-base font-medium",
                "bg-linear-to-t from-stone-600 to-stone-500 text-white",
                "transition-transform hover:scale-105 active:scale-95",
              ])}
            >
              {primaryCTA.label}
            </Link>
            {secondaryCTA && (
              <Link
                to={secondaryCTA.to}
                className={cn([
                  "inline-block rounded-full px-8 py-3 text-base font-medium",
                  "border-color-bright text-fg border",
                  "transition-colors hover:bg-stone-50",
                ])}
              >
                {secondaryCTA.label}
              </Link>
            )}
          </div>
        </header>
        <div className="bg-grid-dark border-color-brand h-48 w-full border"></div>
      </div>
    </div>
  );
}
