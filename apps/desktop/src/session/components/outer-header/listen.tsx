import { useHover } from "@uidotdev/usehooks";
import { MicOff } from "lucide-react";
import { useCallback } from "react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import {
  ActionableTooltipContent,
  useHasTranscript,
  useListenButtonState,
} from "~/session/components/shared";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";

export function ListenButton({ sessionId }: { sessionId: string }) {
  const { shouldRender } = useListenButtonState(sessionId);
  const hasTranscript = useHasTranscript(sessionId);

  if (!shouldRender) {
    return <InMeetingIndicator sessionId={sessionId} />;
  }

  if (hasTranscript) {
    return <StartButton sessionId={sessionId} />;
  }

  return null;
}

function StartButton({ sessionId }: { sessionId: string }) {
  const { isDisabled, warningMessage } = useListenButtonState(sessionId);
  const handleClick = useStartListening(sessionId);
  const openNew = useTabs((state) => state.openNew);

  const handleConfigureAction = useCallback(() => {
    openNew({ type: "ai", state: { tab: "transcription" } });
  }, [openNew]);

  const button = (
    <button
      type="button"
      onClick={handleClick}
      disabled={isDisabled}
      className={cn([
        "inline-flex items-center justify-center rounded-md text-xs font-medium",
        "text-neutral-900",
        "hover:bg-neutral-100/80",
        "gap-1.5",
        "h-7 px-2",
        "disabled:pointer-events-none disabled:opacity-50",
      ])}
    >
      <span className="whitespace-nowrap">Resume listening</span>
    </button>
  );

  if (!warningMessage) {
    return button;
  }

  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <span className="inline-block">{button}</span>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <ActionableTooltipContent
          message={warningMessage}
          action={{
            label: "Configure",
            handleClick: handleConfigureAction,
          }}
        />
      </TooltipContent>
    </Tooltip>
  );
}

function InMeetingIndicator({ sessionId }: { sessionId: string }) {
  const [ref, hovered] = useHover();

  const { mode, stop, amplitude, muted } = useListener((state) => ({
    mode: state.getSessionMode(sessionId),
    stop: state.stop,
    amplitude: state.live.amplitude,
    muted: state.live.muted,
  }));

  const active = mode === "active" || mode === "finalizing";
  const finalizing = mode === "finalizing";

  if (!active) {
    return null;
  }

  return (
    <button
      ref={ref as React.Ref<HTMLButtonElement>}
      type="button"
      onClick={finalizing ? undefined : stop}
      disabled={finalizing}
      className={cn([
        "inline-flex items-center justify-center rounded-md text-sm font-medium",
        finalizing
          ? ["text-neutral-500", "bg-neutral-100", "cursor-wait"]
          : ["text-red-500 hover:text-red-600", "bg-red-50 hover:bg-red-100"],
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
            className={cn([
              "flex items-center gap-1.5",
              hovered ? "hidden" : "flex",
            ])}
          >
            {muted && <MicOff size={14} />}
            <DancingSticks
              amplitude={Math.min(
                Math.hypot(amplitude.mic, amplitude.speaker),
                1,
              )}
              color="#ef4444"
              height={18}
              width={60}
            />
          </div>
          <div
            className={cn([
              "flex items-center gap-1.5",
              hovered ? "flex" : "hidden",
            ])}
          >
            <span className="size-2 rounded-none bg-red-500" />
            <span className="text-xs">Stop</span>
          </div>
        </>
      )}
    </button>
  );
}
