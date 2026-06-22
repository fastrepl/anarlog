import { cn } from "@hypr/utils";

import { useShell } from "~/contexts/shell";

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
          "pointer-events-none absolute bottom-0 left-1/2 inline-flex h-2 w-[min(640px,calc(100cqw_-_2rem))] -translate-x-1/2 items-center overflow-hidden rounded-full border border-transparent bg-black dark:bg-white",
          "[clip-path:inset(0_calc(50%_-_3rem)_0_calc(50%_-_3rem)_round_9999px)]",
          "origin-bottom px-0 text-sm shadow-[0_8px_22px_rgba(0,0,0,0.2)] transition-[clip-path,height,padding,background-color,border-color,box-shadow] duration-200 ease-out dark:shadow-[0_8px_24px_rgba(0,0,0,0.45)]",
          "group-hover/anarlog-chat-cta:border-border/70 group-focus-visible/anarlog-chat-cta:border-border/70 group-hover/anarlog-chat-cta:bg-[#f4f4f5] group-focus-visible/anarlog-chat-cta:bg-[#f4f4f5] dark:group-hover/anarlog-chat-cta:bg-[#202020] dark:group-focus-visible/anarlog-chat-cta:bg-[#202020]",
          "group-hover/anarlog-chat-cta:h-10 group-hover/anarlog-chat-cta:px-4 group-hover/anarlog-chat-cta:[clip-path:inset(0_0_0_0_round_9999px)]",
          "group-focus-visible/anarlog-chat-cta:h-10 group-focus-visible/anarlog-chat-cta:px-4 group-focus-visible/anarlog-chat-cta:[clip-path:inset(0_0_0_0_round_9999px)]",
          "group-focus-visible/anarlog-chat-cta:ring-ring group-focus-visible/anarlog-chat-cta:ring-2 group-focus-visible/anarlog-chat-cta:ring-offset-2",
        ])}
      >
        <span
          aria-hidden="true"
          className={cn([
            "max-w-0 min-w-0 flex-1 truncate text-left opacity-0",
            "group-focus-within/anarlog-chat-cta:text-muted-foreground group-hover/anarlog-chat-cta:text-muted-foreground text-white/55",
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
