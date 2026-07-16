import { useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

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

  if (isChatOpen) {
    return null;
  }

  return (
    <button
      type="button"
      data-chat-cta-trigger
      aria-label={ariaLabel ?? t`Ask Anarlog anything`}
      onClick={handleClick}
      className="group/anarlog-chat-cta relative h-10 w-[180px] max-w-full cursor-text focus-visible:outline-none"
    >
      <span
        data-chat-cta-surface
        aria-hidden="true"
        className={cn([
          "pointer-events-none absolute bottom-0 left-1/2 inline-flex h-2 w-[180px] -translate-x-1/2 items-center overflow-hidden rounded-full border border-transparent dark:h-3",
          "origin-bottom bg-[linear-gradient(180deg,#faf8f6_0%,#e3e1df_100%)] px-0 text-sm shadow-[0_0_0_1px_rgba(0,0,0,0.1),0_4px_12px_rgba(0,0,0,0.16),0_4px_16px_rgba(0,0,0,0.1),inset_0_-1px_0_rgba(0,0,0,0.25),inset_0_1px_0_rgba(255,255,255,0.4)] transition-[width,height,padding,background-color,border-color,box-shadow] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] dark:bg-[linear-gradient(180deg,#211d1d_0%,#574f3b_100%)] dark:shadow-[0_4px_12px_rgba(33,29,29,0.1),inset_0_-1px_0_rgba(0,0,0,0.25),inset_0_1px_0_rgba(255,255,255,0.4)]",
          "group-hover/anarlog-chat-cta:border-border/70 group-focus-visible/anarlog-chat-cta:border-border/70 group-hover/anarlog-chat-cta:bg-[#f4f4f5] group-focus-visible/anarlog-chat-cta:bg-[#f4f4f5] dark:group-hover/anarlog-chat-cta:bg-[#202020] dark:group-focus-visible/anarlog-chat-cta:bg-[#202020]",
          "group-hover/anarlog-chat-cta:shadow-[0_16px_42px_rgba(0,0,0,0.26)] group-focus-visible/anarlog-chat-cta:shadow-[0_16px_42px_rgba(0,0,0,0.26)] dark:group-hover/anarlog-chat-cta:shadow-[0_18px_52px_rgba(0,0,0,0.64)] dark:group-focus-visible/anarlog-chat-cta:shadow-[0_18px_52px_rgba(0,0,0,0.64)]",
          "group-hover/anarlog-chat-cta:h-10 group-hover/anarlog-chat-cta:w-[min(640px,calc(100cqw_-_2rem))] group-hover/anarlog-chat-cta:px-4 dark:group-hover/anarlog-chat-cta:h-10",
          "group-focus-visible/anarlog-chat-cta:h-10 group-focus-visible/anarlog-chat-cta:w-[min(640px,calc(100cqw_-_2rem))] group-focus-visible/anarlog-chat-cta:px-4 dark:group-focus-visible/anarlog-chat-cta:h-10",
          "group-focus-visible/anarlog-chat-cta:ring-ring group-focus-visible/anarlog-chat-cta:ring-2 group-focus-visible/anarlog-chat-cta:ring-offset-2",
        ])}
      >
        <span
          aria-hidden="true"
          className={cn([
            "min-w-0 flex-1 truncate text-left opacity-0",
            "group-focus-within/anarlog-chat-cta:text-muted-foreground group-hover/anarlog-chat-cta:text-muted-foreground text-white/55",
            "transition-opacity duration-100 ease-out",
            "group-hover/anarlog-chat-cta:opacity-100",
            "group-focus-within/anarlog-chat-cta:opacity-100",
          ])}
        >
          {resolvedLabel}
        </span>
      </span>
    </button>
  );
}

export function FloatingChatCTA({ label }: { label?: ReactNode }) {
  return (
    <div className="pointer-events-none absolute bottom-3 left-1/2 z-20 flex h-10 w-[180px] max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center pb-0">
      <div className="pointer-events-auto max-w-full">
        <ChatCTA label={label} />
      </div>
    </div>
  );
}
