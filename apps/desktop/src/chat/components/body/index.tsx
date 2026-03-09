import type { ChatStatus } from "ai";
import { useEffect, useRef } from "react";

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

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, status, error]);

  return (
    <div
      ref={scrollRef}
      className="flex min-h-0 flex-1 flex-col overflow-y-auto"
    >
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
  );
}
