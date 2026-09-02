import { Microphone, MicrophoneSlash } from "@phosphor-icons/react";

import { DropdownMenuItem } from "@anlg/ui/components/ui/dropdown-menu";

import { useListener } from "~/stt/contexts";
import { useStartListeningWithBatchOverride } from "~/stt/useStartListeningWithBatchOverride";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function Listening({
  sessionId,
  resume,
}: {
  sessionId: string;
  resume: boolean;
}) {
  const { mode, stop } = useListener((state) => ({
    mode: state.getSessionMode(sessionId),
    stop: state.stop,
  }));
  const isListening = mode === "active" || mode === "finalizing";
  const isFinalizing = mode === "finalizing";
  const isBatching = mode === "running_batch";
  const startListening = useStartListeningWithBatchOverride(sessionId);

  const handleToggleListening = () => {
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl(
        isListening ? "stop" : "start",
        sessionId,
      );
      return;
    }

    if (isListening) {
      stop();
    } else {
      void startListening();
    }
  };

  const startLabel =
    resume || isBatching ? "Resume listening" : "Start listening";

  return (
    <DropdownMenuItem
      className="cursor-pointer"
      onClick={handleToggleListening}
      disabled={isFinalizing}
    >
      {isListening ? <MicrophoneSlash /> : <Microphone />}
      <span>{isListening ? "Stop listening" : startLabel}</span>
    </DropdownMenuItem>
  );
}
