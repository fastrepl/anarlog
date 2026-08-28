import type { ChatStatus } from "ai";

import { ErrorMessage } from "~/chat/components/message/error";
import { LoadingMessage } from "~/chat/components/message/loading";
import { NormalMessage } from "~/chat/components/message/normal";
import { hasRenderableContent } from "~/chat/components/shared";
import type { AnlgUIMessage } from "~/chat/types";

function isWaitingForAssistantContent(message: AnlgUIMessage | undefined) {
  if (message?.role !== "assistant") {
    return false;
  }

  const lastPart = message.parts[message.parts.length - 1];
  if (!lastPart) {
    return false;
  }

  if (lastPart.type === "step-start") {
    return true;
  }

  const state = "state" in lastPart ? lastPart.state : undefined;
  return (
    lastPart.type.startsWith("tool-") &&
    (state === "output-available" || state === "output-error")
  );
}

export function ChatBodyNonEmpty({
  messages,
  status,
  error,
  onReload,
}: {
  messages: AnlgUIMessage[];
  status: ChatStatus;
  error?: Error;
  onReload?: () => void;
}) {
  const showErrorState = status === "error" && error;
  const lastMessage = messages[messages.length - 1];
  const showLoadingState =
    (status === "submitted" || status === "streaming") &&
    (lastMessage?.role !== "assistant" ||
      !hasRenderableContent(lastMessage) ||
      isWaitingForAssistantContent(lastMessage));

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
