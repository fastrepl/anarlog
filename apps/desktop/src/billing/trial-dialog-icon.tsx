import { cn } from "@hypr/utils";

const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

export function TrialDialogIcon({ state }: { state: "started" | "ended" }) {
  const isStarted = state === "started";

  return (
    <div
      className={cn([
        "relative -my-2 flex size-[92px] items-center justify-center overflow-visible",
        isStarted
          ? "drop-shadow-[0_14px_22px_rgba(180,83,9,0.22)]"
          : "drop-shadow-[0_14px_22px_rgba(0,0,0,0.18)]",
      ])}
    >
      <div
        aria-hidden="true"
        className={cn([
          "absolute size-14 rounded-[18px] blur-md",
          isStarted ? "bg-amber-200/55" : "bg-muted-foreground/30",
        ])}
      />
      <div
        className={cn([
          "relative size-14 rounded-[18px]",
          "shadow-[0_1px_0_rgba(255,255,255,0.75),0_10px_24px_-10px_rgba(0,0,0,0.58)]",
        ])}
      >
        <img
          src={ANARLOG_ICON_SRC}
          alt=""
          aria-hidden="true"
          className={cn([
            "size-full rounded-[18px] object-cover object-center",
            isStarted
              ? "drop-shadow-[0_0_10px_rgba(245,158,11,0.35)]"
              : "opacity-58 brightness-[0.54] grayscale",
          ])}
        />
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 overflow-hidden rounded-[18px]"
        >
          {isStarted ? (
            <span
              className={cn([
                "absolute inset-0 -translate-x-full",
                "animate-shimmer bg-linear-to-r from-transparent via-white/70 to-transparent",
              ])}
            />
          ) : (
            <div className="absolute inset-0 bg-linear-to-b from-neutral-900/5 via-neutral-900/20 to-black/42" />
          )}
        </div>
      </div>
    </div>
  );
}
