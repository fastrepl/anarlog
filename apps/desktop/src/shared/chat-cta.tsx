import { useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

import { cn } from "@anlg/utils";

import { useShell } from "~/contexts/shell";

export function ChatCTA({
  label,
  ariaLabel,
}: {
  label?: ReactNode;
  ariaLabel?: string;
}) {
  const { t } = useLingui();
  const { chat } = useShell();
  const isChatOpen = chat.mode !== "FloatingClosed";
  const resolvedLabel = label ?? t`Ask anything`;

  const handleClick = () => {
    chat.sendEvent({ type: "OPEN" });
  };

  if (chat.mode === "RightPanelOpen") {
    return null;
  }

  return (
    <button
      type="button"
      data-chat-cta-trigger
      aria-hidden={isChatOpen}
      inert={isChatOpen}
      aria-label={ariaLabel ?? t`Ask Anarlog anything`}
      onClick={handleClick}
      className={cn([
        "group/anarlog-chat-cta relative h-10 w-[min(640px,calc(100cqw_-_2rem))] max-w-full cursor-text focus-visible:outline-none",
        isChatOpen && "invisible",
      ])}
    >
      <span
        data-chat-cta-surface
        aria-hidden="true"
        className={cn([
          "rounded-pill border-border/70 pointer-events-none absolute bottom-0 left-1/2 inline-flex h-10 w-[min(640px,calc(100cqw_-_2rem))] -translate-x-1/2 items-center overflow-hidden border",
          "dark:bg-card origin-bottom bg-white px-4 text-sm shadow-[0_6px_20px_rgba(0,0,0,0.10)] transition-[width,height,padding,background-color,border-color,box-shadow] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] dark:shadow-[0_8px_24px_rgba(0,0,0,0.32)]",
          "group-focus-visible/anarlog-chat-cta:ring-ring group-focus-visible/anarlog-chat-cta:ring-2 group-focus-visible/anarlog-chat-cta:ring-offset-2",
        ])}
      >
        <span
          aria-hidden="true"
          className="text-muted-foreground min-w-0 flex-1 truncate text-left"
        >
          {resolvedLabel}
        </span>
      </span>
    </button>
  );
}

export function FloatingChatCTA({ label }: { label?: ReactNode }) {
  return (
    <div className="pointer-events-none absolute bottom-3 left-1/2 z-20 flex h-10 w-[min(640px,calc(100cqw_-_2rem))] max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center pb-0">
      <div className="pointer-events-auto max-w-full">
        <ChatCTA label={label} />
      </div>
    </div>
  );
}
