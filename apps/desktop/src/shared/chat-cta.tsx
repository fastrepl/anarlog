import { useCallback } from "react";

import { useShell } from "~/contexts/shell";

export function ChatCTA({
  label = "Ask about this session",
  openMode = "remember",
}: {
  label?: string;
  openMode?: "remember" | "floating" | "right-panel";
}) {
  const { chat } = useShell();
  const isChatOpen =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";

  const handleClick = useCallback(() => {
    if (isChatOpen) {
      chat.sendEvent({ type: "TOGGLE" });
      return;
    }

    chat.sendEvent(
      openMode === "floating"
        ? { type: "OPEN_FLOATING" }
        : openMode === "right-panel"
          ? { type: "OPEN_RIGHT_PANEL" }
          : { type: "OPEN" },
    );
  }, [chat, isChatOpen, openMode]);

  if (isChatOpen) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={handleClick}
      className="flex items-center gap-2 rounded-full border-2 border-stone-600 bg-stone-800 px-4 py-2 text-sm text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-colors hover:bg-stone-700"
    >
      <img
        src="/assets/char-chat-bubble.svg"
        alt=""
        className="size-4 shrink-0 object-contain invert"
      />
      <span>{label}</span>
    </button>
  );
}
