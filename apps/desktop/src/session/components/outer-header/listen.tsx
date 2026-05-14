import { MicOff } from "lucide-react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import { useListenButtonState } from "~/session/components/shared";
import { useListener } from "~/stt/contexts";

export function ListenButton({ sessionId }: { sessionId: string }) {
  const { shouldRender } = useListenButtonState(sessionId);

  if (!shouldRender) {
    return <InMeetingIndicator sessionId={sessionId} />;
  }

  return null;
}

function InMeetingIndicator({ sessionId }: { sessionId: string }) {
  const { mode, stop, amplitude, muted, degraded } = useListener((state) => ({
    mode: state.getSessionMode(sessionId),
    stop: state.stop,
    amplitude: state.live.amplitude,
    muted: state.live.muted,
    degraded: state.live.degraded,
  }));

  const active = mode === "active" || mode === "finalizing";
  const finalizing = mode === "finalizing";

  if (!active) {
    return null;
  }

  const accent = degraded ? "amber" : "red";
  const colors = {
    red: {
      button: "text-red-500 hover:text-red-600 bg-red-50 hover:bg-red-100",
      sticks: "#ef4444",
      stop: "bg-red-500",
    },
    amber: {
      button:
        "text-amber-500 hover:text-amber-600 bg-amber-50 hover:bg-amber-100",
      sticks: "#f59e0b",
      stop: "bg-amber-500",
    },
  }[accent];

  return (
    <button
      type="button"
      onClick={finalizing ? undefined : stop}
      disabled={finalizing}
      className={cn([
        "group inline-flex items-center justify-center rounded-md text-sm font-medium",
        finalizing
          ? ["text-neutral-500", "bg-neutral-100", "cursor-wait"]
          : [colors.button],
        "h-7 w-20",
        "disabled:pointer-events-none disabled:opacity-50",
      ])}
      aria-label={finalizing ? "Finalizing" : "Stop listening"}
    >
      {finalizing ? (
        <div className="flex items-center gap-1.5">
          <span className="animate-pulse">...</span>
        </div>
      ) : (
        <>
          <div
            className={cn(["flex items-center gap-1.5", "group-hover:hidden"])}
          >
            {muted && <MicOff size={14} />}
            <DancingSticks
              amplitude={Math.min(
                Math.hypot(amplitude.mic, amplitude.speaker),
                1,
              )}
              color={colors.sticks}
              height={18}
              width={60}
            />
          </div>
          <div
            className={cn(["hidden items-center gap-1.5", "group-hover:flex"])}
          >
            <span className={cn(["size-2 rounded-none", colors.stop])} />
            <span className="text-xs">Stop</span>
          </div>
        </>
      )}
    </button>
  );
}
