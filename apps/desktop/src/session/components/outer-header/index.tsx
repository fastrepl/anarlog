import { MicOff } from "lucide-react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import { MetadataButton } from "./metadata";
import { OverflowButton } from "./overflow";

import { useShell } from "~/contexts/shell";
import { useConfigValue } from "~/shared/config";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function OuterHeader({
  sessionId,
  currentView,
  title,
}: {
  sessionId: string;
  currentView: EditorView;
  title?: React.ReactNode;
}) {
  return (
    <div className="flex h-12 w-full items-center">
      <div className="flex w-full min-w-0 items-center justify-between gap-3">
        {title ? <div className="min-w-0 flex-1">{title}</div> : null}
        <div className="flex shrink-0 items-center gap-1">
          <SidebarModeStopButton sessionId={sessionId} />
          <MetadataButton sessionId={sessionId} />
          <OverflowButton sessionId={sessionId} currentView={currentView} />
        </div>
      </div>
    </div>
  );
}

function SidebarModeStopButton({ sessionId }: { sessionId: string }) {
  const { leftsidebar } = useShell();
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const { amplitude, degraded, mode, muted, stop } = useListener((state) => ({
    amplitude: state.live.amplitude,
    degraded: state.live.degraded,
    mode: state.getSessionMode(sessionId),
    muted: state.live.muted,
    stop: state.stop,
  }));
  const active = mode === "active" || mode === "finalizing";
  const finalizing = mode === "finalizing";

  if (!sidebarTimelineEnabled || !leftsidebar.expanded || !active) {
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
        "group inline-flex items-center justify-center rounded-full text-sm font-medium",
        finalizing
          ? ["cursor-wait bg-neutral-100 text-neutral-500"]
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
