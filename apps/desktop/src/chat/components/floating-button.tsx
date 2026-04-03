import { useCallback } from "react";

import { ChatTrigger } from "./trigger";

import { useShell } from "~/contexts/shell";

export function ChatFloatingButton({
  isCaretNearBottom = false,
  bottomAccessoryKind = null,
}: {
  isCaretNearBottom?: boolean;
  bottomAccessoryKind?:
    | "live_transcript"
    | "live_transcript_expanded"
    | "playback"
    | null;
}) {
  const { chat } = useShell();
  const isOpen = chat.mode === "FloatingOpen";

  const handleClickTrigger = useCallback(async () => {
    chat.sendEvent({ type: "OPEN" });
  }, [chat]);

  if (isOpen) {
    return null;
  }

  return (
    <ChatTrigger
      onClick={handleClickTrigger}
      isCaretNearBottom={isCaretNearBottom}
      bottomAccessoryKind={bottomAccessoryKind}
    />
  );
}
