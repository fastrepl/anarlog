import type { ChatStatus } from "ai";

import { ChatBody } from "./body";
import { ContextBar } from "./context-bar";
import { ChatMessageInput } from "./input";

import type { useLanguageModel } from "~/ai/hooks";
import { dedupeByKey, type ContextRef } from "~/chat/context/entities";
import {
  hasSessionContextDragData,
  readSessionContextDragData,
} from "~/chat/context/session-drag";
import type { DisplayEntity } from "~/chat/context/use-chat-context-pipeline";
import type { HyprUIMessage } from "~/chat/types";

export function ChatContent({
  layout = "floating",
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
  onDraftContentChange,
  onDraftContextRefsChange,
  isSystemPromptReady,
  children,
}: {
  layout?: "floating" | "right-panel";
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
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  onDraftContextRefsChange?: (refs: ContextRef[]) => void;
  isSystemPromptReady: boolean;
  children?: React.ReactNode;
}) {
  const isModelConfigured = !!model;
  const isFloating = layout === "floating";
  const disabled = !isSystemPromptReady;
  const mergeContextRefs = (contextRefs?: ContextRef[]) =>
    contextRefs ? dedupeByKey([pendingRefs, contextRefs]) : pendingRefs;
  const handleDragOver = (event: React.DragEvent<HTMLDivElement>) => {
    if (!onAddContextEntity || !hasSessionContextDragData(event.dataTransfer)) {
      return;
    }

    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  };
  const handleDrop = (event: React.DragEvent<HTMLDivElement>) => {
    if (!onAddContextEntity) {
      return;
    }

    const contextRef = readSessionContextDragData(event.dataTransfer);

    if (!contextRef) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    onAddContextEntity(contextRef);
  };

  return (
    <div
      className={
        isFloating
          ? "flex min-h-0 shrink-0 flex-col overflow-hidden"
          : "flex min-h-0 flex-1 flex-col overflow-hidden"
      }
      data-chat-content
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {children ?? (
        <ChatBody
          messages={messages}
          status={status}
          error={error}
          onReload={regenerate}
          isModelConfigured={isModelConfigured}
          onSendMessage={(content, parts, contextRefs) => {
            handleSendMessage(
              content,
              parts,
              sendMessage,
              mergeContextRefs(contextRefs),
            );
          }}
        />
      )}
      {isModelConfigured && (
        <>
          <ContextBar
            entities={contextEntities}
            onRemoveEntity={onRemoveContextEntity}
            onAddEntity={onAddContextEntity}
          />
          <ChatMessageInput
            draftKey={sessionId}
            disabled={disabled}
            hasContextBar={contextEntities.length > 0}
            onSendMessage={(content, parts, contextRefs) => {
              handleSendMessage(
                content,
                parts,
                sendMessage,
                mergeContextRefs(contextRefs),
              );
            }}
            onDraftContentChange={onDraftContentChange}
            onContextRefsChange={onDraftContextRefsChange}
            isStreaming={status === "streaming" || status === "submitted"}
            onStop={stop}
          />
        </>
      )}
    </div>
  );
}
