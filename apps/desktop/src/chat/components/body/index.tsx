import type { ChatStatus } from "ai";
import { ChevronDownIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { ChatBodyEmpty } from "./empty";
import { ChatBodyNonEmpty } from "./non-empty";
import { useChatAutoScroll } from "./use-chat-auto-scroll";

import type { HyprUIMessage } from "~/chat/types";
import { useShell } from "~/contexts/shell";

export function ChatBody({
  messages,
  status,
  error,
  onReload,
  isModelConfigured = true,
  hasContext = false,
  onSendMessage,
}: {
  messages: HyprUIMessage[];
  status: ChatStatus;
  error?: Error;
  onReload?: () => void;
  isModelConfigured?: boolean;
  hasContext?: boolean;
  onSendMessage?: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
  ) => void;
}) {
  const { chat } = useShell();
  const {
    contentRef,
    isAtBottom,
    scrollRef,
    scrollToBottom,
    showGoToRecent,
    updateAutoScrollState,
    handleWheel,
  } = useChatAutoScroll(status);

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={scrollRef}
        onScroll={updateAutoScrollState}
        onWheel={handleWheel}
        className="flex min-h-0 flex-1 flex-col overflow-y-auto"
      >
        <div
          ref={contentRef}
          className={cn([
            "flex min-h-full flex-1 flex-col py-3",
            chat.mode === "FloatingOpen" ? "px-4" : "px-2",
          ])}
        >
          <div className="flex-1" />
          {messages.length === 0 ? (
            <ChatBodyEmpty
              isModelConfigured={isModelConfigured}
              hasContext={hasContext}
              onSendMessage={onSendMessage}
            />
          ) : (
            <ChatBodyNonEmpty
              messages={messages}
              status={status}
              error={error}
              onReload={onReload}
            />
          )}
        </div>
      </div>
      {messages.length > 0 && showGoToRecent && !isAtBottom && (
        <Button
          onClick={scrollToBottom}
          size="sm"
          className="absolute bottom-3 left-1/2 z-20 flex -translate-x-1/2 transform items-center gap-1 rounded-full border border-neutral-200 bg-white text-neutral-700 shadow-xs hover:bg-neutral-50"
          variant="outline"
        >
          <ChevronDownIcon size={12} />
          <span className="text-xs">Go to recent</span>
        </Button>
      )}
    </div>
  );
}
