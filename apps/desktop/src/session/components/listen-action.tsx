import * as stylex from "@stylexjs/stylex";
import { useCallback } from "react";

import { Spinner } from "@anlg/ui/components/ui/spinner";

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
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function ListenActionButton({ sessionId }: { sessionId: string }) {
  const { shouldRender, isDisabled, warningMessage, recoverySettingsTab } =
    useListenButtonState(sessionId);
  const loading = useListener(
    (state) => state.live.loading && state.live.sessionId === sessionId,
  );

  if (loading) {
    return <StopListeningButton sessionId={sessionId} />;
  }

  if (!shouldRender) {
    return null;
  }

  return (
    <StartListeningButton
      sessionId={sessionId}
      isDisabled={isDisabled}
      warningMessage={warningMessage}
      recoverySettingsTab={recoverySettingsTab}
    />
  );
}

function StopListeningButton({ sessionId }: { sessionId: string }) {
  const stop = useListener((state) => state.stop);

  const handleStop = useCallback(() => {
    // Starts are proxied to the main window, so stops must be too — a local
    // stop cannot end a session the main window owns.
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl("stop", sessionId);
      return;
    }

    stop();
  }, [sessionId, stop]);

  return (
    <FloatingButton onClick={handleStop}>
      <Spinner />
    </FloatingButton>
  );
}

function StartListeningButton({
  sessionId,
  isDisabled,
  warningMessage,
  recoverySettingsTab,
}: {
  sessionId: string;
  isDisabled: boolean;
  warningMessage: string;
  recoverySettingsTab: "permissions" | null;
}) {
  const startListening = useStartListening(sessionId);
  const openNew = useTabs((state) => state.openNew);
  const noteHasContent = useCurrentNoteHasContent(sessionId, { type: "raw" });

  const handleStart = useCallback(() => {
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl("start", sessionId);
      return;
    }

    void startListening();
  }, [sessionId, startListening]);

  const handleConfigure = useCallback(() => {
    if (recoverySettingsTab) {
      openNew({ type: "settings", state: { tab: recoverySettingsTab } });
    }
  }, [openNew, recoverySettingsTab]);

  return (
    <div>
      <OptionsMenu
        sessionId={sessionId}
        disabled={isDisabled}
        warningMessage={warningMessage}
        hideUploadActions={noteHasContent}
        onConfigure={recoverySettingsTab ? handleConfigure : undefined}
      >
        <FloatingButton
          onClick={handleStart}
          disabled={isDisabled}
          sx={styles.startButton}
          tooltip={
            warningMessage
              ? {
                  side: "top",
                  content: (
                    <ActionableTooltipContent
                      message={warningMessage}
                      action={
                        recoverySettingsTab
                          ? {
                              label: "Configure",
                              handleClick: handleConfigure,
                            }
                          : undefined
                      }
                    />
                  ),
                }
              : undefined
          }
        >
          <span {...stylex.props(styles.startLabel)}>
            <RecordingIcon /> Start listening
          </span>
        </FloatingButton>
      </OptionsMenu>
    </div>
  );
}

const styles = stylex.create({
  startButton: {
    gap: "0.5rem",
    justifyContent: "flex-start",
    paddingLeft: "0.75rem",
    paddingRight: "1.75rem",
    width: "148px",
  },
  startLabel: {
    alignItems: "center",
    display: "flex",
    gap: "0.375rem",
  },
});
