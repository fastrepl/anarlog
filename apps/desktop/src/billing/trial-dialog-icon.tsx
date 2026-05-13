import { cn } from "@hypr/utils";

const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

export function TrialDialogIcon({ state }: { state: "started" | "ended" }) {
  const isStarted = state === "started";

  return (
    <div
      className={cn([
        "relative mb-2 flex size-12 items-center justify-center overflow-hidden rounded-[14px]",
      ])}
    >
      <img
        src={ANARLOG_ICON_SRC}
        alt=""
        aria-hidden="true"
        className={cn([
          "size-12 object-contain object-center",
          isStarted
            ? "drop-shadow-[0_0_10px_rgba(245,158,11,0.45)]"
            : "opacity-75 brightness-[0.58] grayscale",
        ])}
      />
      {isStarted ? (
        <span
          aria-hidden="true"
          className={cn([
            "absolute inset-0 -translate-x-full",
            "animate-shimmer bg-linear-to-r from-transparent via-white/70 to-transparent",
          ])}
        />
      ) : (
        <>
          <div
            aria-hidden="true"
            className="absolute inset-0 bg-linear-to-b from-white/20 via-neutral-700/10 to-black/35"
          />
          <div
            aria-hidden="true"
            className="absolute inset-x-1 bottom-1 h-1/2 rounded-b-[12px] bg-black/15 blur-sm"
          />
        </>
      )}
    </div>
  );
}
