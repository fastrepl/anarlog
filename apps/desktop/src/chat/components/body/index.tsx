import type { ChatStatus } from "ai";
import { ChevronDownIcon } from "lucide-react";
import { type WheelEvent, useEffect, useRef, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";

import { ChatBodyEmpty } from "./empty";
import { ChatBodyNonEmpty } from "./non-empty";

import type { HyprUIMessage } from "~/chat/types";

export function ChatBody({
  messages,
  status,
  error,
  onReload,
  isModelConfigured = true,
  onSendMessage,
}: {
  messages: HyprUIMessage[];
  status: ChatStatus;
  error?: Error;
  onReload?: () => void;
  isModelConfigured?: boolean;
  onSendMessage?: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
  ) => void;
}) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const shouldAutoScrollRef = useRef(true);
  const previousIsGeneratingRef = useRef(false);
  const pendingUserScrollIntentRef = useRef(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [showGoToRecent, setShowGoToRecent] = useState(false);
  const isGenerating = status === "submitted" || status === "streaming";

  const scrollToBottom = () => {
    if (!scrollRef.current) {
      return;
    }

    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    shouldAutoScrollRef.current = true;
    pendingUserScrollIntentRef.current = false;
    setIsAtBottom(true);
    setShowGoToRecent(false);
  };

  const updateAutoScrollState = () => {
    if (!scrollRef.current) {
      return;
    }

    const { scrollTop, clientHeight, scrollHeight } = scrollRef.current;
    const distanceFromBottom = scrollHeight - (scrollTop + clientHeight);
    const isAtBottom = distanceFromBottom <= 24;
    setIsAtBottom(isAtBottom);

    if (isAtBottom) {
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
      return;
    }

    if (pendingUserScrollIntentRef.current) {
      shouldAutoScrollRef.current = false;
    }
  };

  const handleWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY > 0 && !isAtBottom) {
      setShowGoToRecent(true);
      return;
    }

    if (event.deltaY < 0) {
      setShowGoToRecent(false);
    }

    if (!isGenerating || event.deltaY >= 0) {
      return;
    }

    pendingUserScrollIntentRef.current = true;
  };

  useEffect(() => {
    if (isGenerating && !previousIsGeneratingRef.current) {
      shouldAutoScrollRef.current = true;
      pendingUserScrollIntentRef.current = false;
      setShowGoToRecent(false);
    }

    previousIsGeneratingRef.current = isGenerating;

    if (shouldAutoScrollRef.current) {
      scrollToBottom();
    }
  }, [messages, status, error, isGenerating]);

  useEffect(() => {
    if (!contentRef.current) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (shouldAutoScrollRef.current) {
        scrollToBottom();
      }
    });

    observer.observe(contentRef.current);

    return () => observer.disconnect();
  }, []);

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={scrollRef}
        onScroll={updateAutoScrollState}
        onWheel={handleWheel}
        className="flex min-h-0 flex-1 flex-col overflow-y-auto"
      >
        <div ref={contentRef} className="flex min-h-full flex-1 flex-col">
          <div className="flex-1" />
          {messages.length === 0 ? (
            <ChatBodyEmpty
              isModelConfigured={isModelConfigured}
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
