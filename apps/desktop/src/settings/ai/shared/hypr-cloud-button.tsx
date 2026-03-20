import { cn } from "@hypr/utils";

export function HyprProviderRow({ children }: { children: React.ReactNode }) {
  return (
    <div
      className={cn([
        "flex flex-col gap-3",
        "rounded-md border bg-white px-3 py-2",
      ])}
    >
      {children}
    </div>
  );
}

export function HyprCloudCTAButton({
  isPro,
  canStartTrial,
  highlight,
  onClick,
}: {
  isPro: boolean;
  canStartTrial: boolean | undefined;
  highlight?: boolean;
  onClick: () => void;
}) {
  const buttonLabel = isPro
    ? "Ready to use"
    : canStartTrial
      ? "Start Free Trial"
      : "Upgrade to Pro";

  const showShimmer = highlight && !isPro;

  return (
    <button
      onClick={onClick}
      className={cn([
        "relative h-8.5 w-fit overflow-hidden",
        "rounded-full px-4 text-center font-mono text-xs",
        "transition-all duration-150",
        isPro
          ? "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs hover:shadow-md"
          : "bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
      ])}
    >
      {showShimmer && (
        <div
          className={cn([
            "absolute inset-0 -translate-x-full",
            "bg-linear-to-r from-transparent via-white/20 to-transparent",
            "animate-shimmer",
          ])}
        />
      )}
      <span className="relative z-10">{buttonLabel}</span>
    </button>
  );
}
