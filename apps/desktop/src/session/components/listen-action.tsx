import { useCallback } from "react";

import { Spinner } from "@hypr/ui/components/ui/spinner";

import { OptionsMenu } from "./floating/options-menu";
import { ActionableTooltipContent, FloatingButton } from "./floating/shared";
import { RecordingIcon, useListenButtonState } from "./shared";

import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";

export function ListenActionButton({ sessionId }: { sessionId: string }) {
  const { shouldRender, isDisabled, warningMessage } =
    useListenButtonState(sessionId);
  const { loading, stop } = useListener((state) => ({
    loading: state.live.loading && state.live.sessionId === sessionId,
    stop: state.stop,
  }));
  const startListening = useStartListening(sessionId);
  const openNew = useTabs((state) => state.openNew);

  const handleConfigure = useCallback(() => {
    startListening();
    openNew({ type: "settings", state: { tab: "transcription" } });
  }, [startListening, openNew]);

  if (loading) {
    return (
      <FloatingButton onClick={stop}>
        <Spinner />
      </FloatingButton>
    );
  }

  if (!shouldRender) {
    return null;
  }

  return (
    <div>
      <OptionsMenu
        sessionId={sessionId}
        disabled={isDisabled}
        warningMessage={warningMessage}
        onConfigure={handleConfigure}
      >
        <FloatingButton
          onClick={startListening}
          disabled={isDisabled}
          className="border-primary bg-primary text-primary-foreground hover:bg-primary/90 dark:border-border dark:bg-accent dark:text-accent-foreground dark:hover:bg-accent/90 dark:ring-border/70 w-[148px] justify-start gap-2 pr-7 pl-3 shadow-[0_4px_14px_rgba(87,83,78,0.4)] dark:shadow-[0_10px_30px_rgba(0,0,0,0.45)] dark:ring-1"
          tooltip={
            warningMessage
              ? {
                  side: "top",
                  content: (
                    <ActionableTooltipContent
                      message={warningMessage}
                      action={{
                        label: "Configure",
                        handleClick: handleConfigure,
                      }}
                    />
                  ),
                }
              : undefined
          }
        >
          <span className="flex items-center gap-1.5">
            <RecordingIcon /> Start listening
          </span>
        </FloatingButton>
      </OptionsMenu>
    </div>
  );
}
