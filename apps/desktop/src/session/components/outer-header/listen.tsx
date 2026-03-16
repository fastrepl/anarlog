import { useCallback } from "react";

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
import { useStartListening } from "~/stt/useStartListening";

export function ListenButton({ sessionId }: { sessionId: string }) {
  const { shouldRender } = useListenButtonState(sessionId);
  const hasTranscript = useHasTranscript(sessionId);

  if (shouldRender && hasTranscript) {
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
