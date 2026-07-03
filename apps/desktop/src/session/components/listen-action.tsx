import { useCallback } from "react";

import { Spinner } from "@hypr/ui/components/ui/spinner";

import { OptionsMenu } from "./floating/options-menu";
import { ActionableTooltipContent, FloatingButton } from "./floating/shared";
import {
  RecordingIcon,
  useCurrentNoteHasContent,
  useListenButtonState,
} from "./shared";

import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";

export function ListenActionButton({ sessionId }: { sessionId: string }) {
  const { shouldRender, isDisabled, warningMessage } =
    useListenButtonState(sessionId);
  const loading = useListener(
    (state) => state.live.loading && state.live.sessionId === sessionId,
  );

  if (loading) {
    return <StopListeningButton />;
  }

  if (!shouldRender) {
    return null;
  }

  return (
    <StartListeningButton
      sessionId={sessionId}
      isDisabled={isDisabled}
      warningMessage={warningMessage}
    />
  );
}

function StopListeningButton() {
  const stop = useListener((state) => state.stop);

  return (
    <FloatingButton onClick={stop}>
      <Spinner />
    </FloatingButton>
  );
}

function StartListeningButton({
  sessionId,
  isDisabled,
  warningMessage,
}: {
  sessionId: string;
  isDisabled: boolean;
  warningMessage: string;
}) {
  const startListening = useStartListening(sessionId);
  const openNew = useTabs((state) => state.openNew);
  const noteHasContent = useCurrentNoteHasContent(sessionId, { type: "raw" });

  const handleConfigure = useCallback(() => {
    startListening();
    openNew({ type: "settings", state: { tab: "transcription" } });
  }, [startListening, openNew]);

  return (
    <div>
      <OptionsMenu
        sessionId={sessionId}
        disabled={isDisabled}
        warningMessage={warningMessage}
        hideUploadActions={noteHasContent}
        onConfigure={handleConfigure}
      >
        <FloatingButton
          onClick={startListening}
          disabled={isDisabled}
          className="w-[148px] justify-start gap-2 pr-7 pl-3"
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
