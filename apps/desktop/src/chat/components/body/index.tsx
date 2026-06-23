import type { ChatStatus } from "ai";
import { ChevronDownIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { ChatBodyEmpty } from "./empty";
import { ChatBodyNonEmpty } from "./non-empty";
import { useChatAutoScroll } from "./use-chat-auto-scroll";

import type { ContextRef } from "~/chat/context/entities";
import { chatFloatingControlClassNames } from "~/chat/surface";
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
    contextRefs?: ContextRef[],
  ) => void;
}) {
  const { chat } = useShell();
  const isRightPanel = chat.mode === "RightPanelOpen";
  const isFloating = chat.mode === "FloatingOpen";
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
    <div
      className={cn([
        "relative flex min-h-0 flex-col",
        isRightPanel ? "flex-1" : "flex-auto",
      ])}
    >
      <div
        ref={scrollRef}
        onScroll={updateAutoScrollState}
        onWheel={handleWheel}
        className={cn([
          "flex min-h-0 flex-col overflow-y-auto",
          isRightPanel ? "flex-1" : "max-h-[min(36rem,70vh)] flex-auto",
        ])}
      >
        <div
          ref={contentRef}
          className={cn([
            "flex flex-col",
            isRightPanel && "min-h-full flex-1",
            isRightPanel ? "px-3 py-5" : "px-3 py-3",
          ])}
        >
          {!isFloating && <div className="flex-1" />}
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
          className={cn([
            "absolute bottom-3 left-1/2 z-20 flex -translate-x-1/2 transform items-center gap-1 rounded-full border shadow-xs",
            chatFloatingControlClassNames(),
          ])}
          variant="outline"
        >
          <ChevronDownIcon size={12} />
          <span className="text-xs">Go to recent</span>
        </Button>
      )}
    </div>
  );
}
