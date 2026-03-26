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
    loading: state.live.loading,
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
          className="justify-center gap-2 border-stone-600 bg-stone-800 pr-8 pl-3 text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] hover:bg-stone-700 lg:pr-10 lg:pl-4"
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
          <span className="flex items-center gap-2 pl-3">
            <RecordingIcon /> Start listening
          </span>
        </FloatingButton>
      </OptionsMenu>
    </div>
  );
}
