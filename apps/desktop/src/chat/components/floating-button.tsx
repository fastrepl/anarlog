import { useCallback } from "react";

import { ChatTrigger } from "./trigger";

import { useShell } from "~/contexts/shell";

export function ChatFloatingButton({
  isCaretNearBottom = false,
  showTimeline = false,
}: {
  isCaretNearBottom?: boolean;
  showTimeline?: boolean;
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
      showTimeline={showTimeline}
    />
  );
}
