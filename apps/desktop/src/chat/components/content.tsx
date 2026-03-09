import type { ChatStatus } from "ai";

import { ChatBody } from "./body";
import { ContextBar } from "./context-bar";
import { ChatMessageInput, type McpIndicator } from "./input";

import type { useLanguageModel } from "~/ai/hooks";
import type { ContextRef } from "~/chat/context/entities";
import type { DisplayEntity } from "~/chat/context/use-chat-context-pipeline";
import type { HyprUIMessage } from "~/chat/types";

export function ChatContent({
  sessionId,
  messages,
  sendMessage,
  regenerate,
  stop,
  status,
  error,
  model,
  handleSendMessage,
  contextEntities,
  pendingRefs,
  onRemoveContextEntity,
  onAddContextEntity,
  isSystemPromptReady,
  mcpIndicator,
  children,
}: {
  sessionId: string;
  messages: HyprUIMessage[];
  sendMessage: (message: HyprUIMessage) => void;
  regenerate: () => void;
  stop: () => void;
  status: ChatStatus;
  error?: Error;
  model: ReturnType<typeof useLanguageModel>;
  handleSendMessage: (
    content: string,
    parts: HyprUIMessage["parts"],
    sendMessage: (message: HyprUIMessage) => void,
    contextRefs?: ContextRef[],
  ) => void;
  contextEntities: DisplayEntity[];
  pendingRefs: ContextRef[];
  onRemoveContextEntity?: (key: string) => void;
  onAddContextEntity?: (ref: ContextRef) => void;
  isSystemPromptReady: boolean;
  mcpIndicator?: McpIndicator;
  children?: React.ReactNode;
}) {
  const disabled = !model || !isSystemPromptReady;

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      {children ?? (
        <ChatBody
          messages={messages}
          status={status}
          error={error}
          onReload={regenerate}
          isModelConfigured={!!model}
          onSendMessage={(content, parts) => {
            handleSendMessage(content, parts, sendMessage, pendingRefs);
          }}
        />
      )}
      <ContextBar
        entities={contextEntities}
        onRemoveEntity={onRemoveContextEntity}
        onAddEntity={onAddContextEntity}
      />
      <ChatMessageInput
        draftKey={sessionId}
        disabled={disabled}
        hasContextBar={contextEntities.length > 0}
        onSendMessage={(content, parts) => {
          handleSendMessage(content, parts, sendMessage, pendingRefs);
        }}
        isStreaming={status === "streaming" || status === "submitted"}
        onStop={stop}
        mcpIndicator={mcpIndicator}
      />
    </div>
  );
}
