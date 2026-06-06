import { cn } from "@hypr/utils";

export function HyprProviderRow({ children }: { children: React.ReactNode }) {
  return (
    <div
      className={cn([
        "flex flex-col gap-3",
        "bg-card rounded-md border px-3 py-2",
      ])}
    >
      {children}
    </div>
  );
}

export function HyprCloudCTAButton({
  isPaid,
  canStartTrial,
  highlight,
  onClick,
}: {
  isPaid: boolean;
  canStartTrial: boolean | undefined;
  highlight?: boolean;
  onClick: () => void;
}) {
  const buttonLabel = isPaid
    ? "Ready to use"
    : canStartTrial
      ? "Start Free Trial"
      : "Upgrade";

  const showShimmer = highlight && !isPaid;

  return (
    <button
      onClick={onClick}
      className={cn([
        "relative h-8.5 w-fit overflow-hidden",
        "rounded-full px-4 text-center font-mono text-xs",
        "transition-all duration-150",
        isPaid
          ? "from-muted to-accent text-foreground bg-linear-to-t shadow-xs hover:shadow-md"
          : "bg-primary text-primary-foreground hover:bg-primary/90 shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
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
