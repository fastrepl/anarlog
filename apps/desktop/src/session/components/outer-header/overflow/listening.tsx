import { MicIcon, MicOffIcon } from "lucide-react";

import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";
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
  const startListening = useStartListening(sessionId);

  const handleToggleListening = () => {
    if (isBatching) {
      return;
    }

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
      startListening();
    }
  };

  const startLabel = resume ? "Resume listening" : "Start listening";

  return (
    <DropdownMenuItem
      className="cursor-pointer"
      onClick={handleToggleListening}
      disabled={isFinalizing || isBatching}
    >
      {isListening ? <MicOffIcon /> : <MicIcon />}
      <span>
        {isBatching
          ? "Batch processing"
          : isListening
            ? "Stop listening"
            : startLabel}
      </span>
    </DropdownMenuItem>
  );
}
