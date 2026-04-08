import { useCallback } from "react";

import { useShell } from "~/contexts/shell";

export function ChatCTA() {
  const { chat } = useShell();

  const handleClick = useCallback(() => {
    const isChatOpen =
      chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";
    chat.sendEvent(isChatOpen ? { type: "TOGGLE" } : { type: "OPEN" });
  }, [chat]);

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
      <span>Ask about this session...</span>
    </button>
  );
}
