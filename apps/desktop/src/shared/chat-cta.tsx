import { MessageCircle } from "lucide-react";

import { cn } from "@hypr/utils";

import { useShell } from "~/contexts/shell";

export function ChatCTA({
  label = "Ask Anarlog anything",
}: {
  label?: string;
}) {
  const { chat } = useShell();
  const isChatOpen = chat.mode !== "FloatingClosed";

  const handleClick = () => {
    chat.sendEvent({ type: "OPEN" });
  };

  if (isChatOpen) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={handleClick}
      className={cn([
        "border-primary bg-primary inline-flex max-w-full items-center gap-2 rounded-full border-2",
        "text-primary-foreground px-4 py-2 text-sm shadow-[0_6px_20px_rgba(0,0,0,0.45)]",
        "hover:bg-primary/90 transition-colors",
      ])}
    >
      <MessageCircle className="size-4 shrink-0" aria-hidden="true" />
      <span className="min-w-0 truncate">{label}</span>
    </button>
  );
}

export function FloatingChatCTA({ label }: { label?: string }) {
  return (
    <div className="pointer-events-none absolute bottom-0 left-1/2 z-20 flex h-14 max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center pb-4">
      <div className="pointer-events-auto max-w-full">
        <ChatCTA label={label} />
      </div>
    </div>
  );
}
