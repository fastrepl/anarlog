import type { ChatStatus } from "ai";

import { ErrorMessage } from "~/chat/components/message/error";
import { LoadingMessage } from "~/chat/components/message/loading";
import { NormalMessage } from "~/chat/components/message/normal";
import { hasRenderableContent } from "~/chat/components/shared";
import type { HyprUIMessage } from "~/chat/types";

export function ChatBodyNonEmpty({
  messages,
  status,
  error,
  onReload,
}: {
  messages: HyprUIMessage[];
  status: ChatStatus;
  error?: Error;
  onReload?: () => void;
}) {
  const showErrorState = status === "error" && error;
  const lastMessage = messages[messages.length - 1];
  const showLoadingState =
    (status === "submitted" || status === "streaming") &&
    (lastMessage?.role !== "assistant" || !hasRenderableContent(lastMessage));

  let lastAssistantIndex = -1;
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === "assistant") {
      lastAssistantIndex = i;
      break;
    }
  }

  return (
    <div className="flex flex-col">
      {messages.map((message, index) => (
        <NormalMessage
          key={message.id}
          message={message}
          handleReload={
            message.role === "assistant" &&
            index === lastAssistantIndex &&
            onReload
              ? onReload
              : undefined
          }
        />
      ))}
      {showLoadingState && <LoadingMessage />}
      {showErrorState && <ErrorMessage error={error} onRetry={onReload} />}
    </div>
  );
}
