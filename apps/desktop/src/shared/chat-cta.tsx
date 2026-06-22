import { MessageCircle } from "lucide-react";

import { cn } from "@hypr/utils";

import { useShell } from "~/contexts/shell";
import { floatingActionSurfaceClassName } from "~/shared/floating-action-surface";

export function ChatCTA({
  label = "Ask anything",
  ariaLabel = "Ask Anarlog anything",
}: {
  label?: string;
  ariaLabel?: string;
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
      aria-label={ariaLabel}
      onClick={handleClick}
      className="group/anarlog-chat-cta relative h-10 w-40 max-w-full cursor-text focus-visible:outline-none"
    >
      <span
        data-chat-cta-surface
        aria-hidden="true"
        className={cn([
          "pointer-events-none absolute bottom-0 left-1/2 inline-flex h-[10px] w-24 max-w-[calc(100vw-2rem)] -translate-x-1/2 items-center overflow-hidden rounded-full border-2",
          "origin-bottom px-0 text-sm transition-[width,height,padding,background-color,border-color,box-shadow] duration-200 ease-out",
          "group-hover/anarlog-chat-cta:h-10 group-hover/anarlog-chat-cta:w-[640px] group-hover/anarlog-chat-cta:px-4",
          "group-focus-visible/anarlog-chat-cta:h-10 group-focus-visible/anarlog-chat-cta:w-[640px] group-focus-visible/anarlog-chat-cta:px-4",
          "group-focus-visible/anarlog-chat-cta:ring-ring group-focus-visible/anarlog-chat-cta:ring-2 group-focus-visible/anarlog-chat-cta:ring-offset-2",
          floatingActionSurfaceClassName,
        ])}
      >
        <MessageCircle
          className={cn([
            "text-background/55 dark:text-primary/50 size-4 shrink-0 opacity-0 transition-opacity duration-150",
            "group-focus-within/anarlog-chat-cta:opacity-100 group-hover/anarlog-chat-cta:opacity-100",
          ])}
          aria-hidden="true"
        />
        <span
          aria-hidden="true"
          className={cn([
            "ml-2 max-w-0 min-w-0 flex-1 truncate text-left opacity-0",
            "text-background/55 dark:text-primary/50",
            "transition-[max-width,opacity] duration-200 ease-out",
            "group-hover/anarlog-chat-cta:max-w-full group-hover/anarlog-chat-cta:opacity-100",
            "group-focus-within/anarlog-chat-cta:max-w-full group-focus-within/anarlog-chat-cta:opacity-100",
          ])}
        >
          {label}
        </span>
      </span>
    </button>
  );
}

export function FloatingChatCTA({ label }: { label?: string }) {
  return (
    <div className="pointer-events-none absolute bottom-3 left-1/2 z-20 flex h-10 w-40 max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center pb-0">
      <div className="pointer-events-auto max-w-full">
        <ChatCTA label={label} />
      </div>
    </div>
  );
}
