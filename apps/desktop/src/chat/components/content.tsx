import type { ChatStatus } from "ai";
import { CornerDownRightIcon, Trash2Icon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

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
import { id } from "~/shared/utils";

type QueuedChatMessage = {
  id: string;
  content: string;
  parts: HyprUIMessage["parts"];
  contextRefs: ContextRef[];
};

const EMPTY_QUEUED_MESSAGES: readonly QueuedChatMessage[] = Object.freeze([]);

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
  sendMessage: (
    message: HyprUIMessage,
    options?: { chatGroupId?: string },
  ) => void;
  regenerate: () => void;
  stop: () => void;
  status: ChatStatus;
  error?: Error;
  model: ReturnType<typeof useLanguageModel>;
  handleSendMessage: (
    content: string,
    parts: HyprUIMessage["parts"],
    sendMessage: (
      message: HyprUIMessage,
      options?: { chatGroupId?: string },
    ) => void,
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
  const isBusy = status === "submitted" || status === "streaming";
  const [queueState, setQueueState] = useState<{
    sessionId: string;
    messages: QueuedChatMessage[];
  }>(() => ({ sessionId, messages: [] }));
  const dequeueInFlightRef = useRef(false);
  const queuedMessages =
    queueState.sessionId === sessionId
      ? queueState.messages
      : EMPTY_QUEUED_MESSAGES;
  const mergeContextRefs = useCallback(
    (contextRefs?: ContextRef[]) =>
      contextRefs ? dedupeByKey([pendingRefs, contextRefs]) : pendingRefs,
    [pendingRefs],
  );
  const setQueuedMessages = useCallback(
    (
      next:
        | QueuedChatMessage[]
        | ((messages: QueuedChatMessage[]) => QueuedChatMessage[]),
    ) => {
      setQueueState((prev) => {
        const currentMessages =
          prev.sessionId === sessionId ? prev.messages : [];
        return {
          sessionId,
          messages: typeof next === "function" ? next(currentMessages) : next,
        };
      });
    },
    [sessionId],
  );
  const submitOrQueueMessage = useCallback(
    (
      content: string,
      parts: HyprUIMessage["parts"],
      contextRefs?: ContextRef[],
    ) => {
      const mergedContextRefs = mergeContextRefs(contextRefs);

      if (isBusy) {
        setQueuedMessages((messages) => [
          ...messages,
          {
            id: id(),
            content,
            parts,
            contextRefs: mergedContextRefs,
          },
        ]);
        return;
      }

      handleSendMessage(content, parts, sendMessage, mergedContextRefs);
    },
    [
      handleSendMessage,
      isBusy,
      mergeContextRefs,
      sendMessage,
      setQueuedMessages,
    ],
  );
  const removeQueuedMessage = useCallback(
    (queuedMessageId: string) => {
      setQueuedMessages((messages) =>
        messages.filter((message) => message.id !== queuedMessageId),
      );
    },
    [setQueuedMessages],
  );

  useEffect(() => {
    if (isBusy) {
      dequeueInFlightRef.current = false;
      return;
    }

    if (
      status !== "ready" ||
      queuedMessages.length === 0 ||
      dequeueInFlightRef.current
    ) {
      return;
    }

    const [nextMessage] = queuedMessages;
    dequeueInFlightRef.current = true;
    setQueuedMessages((messages) => messages.slice(1));
    try {
      handleSendMessage(
        nextMessage.content,
        nextMessage.parts,
        sendMessage,
        nextMessage.contextRefs,
      );
    } finally {
      dequeueInFlightRef.current = false;
    }
  }, [
    handleSendMessage,
    isBusy,
    queuedMessages,
    sendMessage,
    setQueuedMessages,
    status,
  ]);

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
          ? "flex max-h-full min-h-0 flex-col overflow-hidden"
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
          onSendMessage={submitOrQueueMessage}
        />
      )}
      {isModelConfigured && (
        <>
          <ContextBar
            entities={contextEntities}
            onRemoveEntity={onRemoveContextEntity}
          />
          <ChatQueue
            messages={queuedMessages}
            onRemoveMessage={removeQueuedMessage}
          />
          <ChatMessageInput
            draftKey={sessionId}
            disabled={disabled}
            onSendMessage={submitOrQueueMessage}
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

function ChatQueue({
  messages,
  onRemoveMessage,
}: {
  messages: readonly QueuedChatMessage[];
  onRemoveMessage: (messageId: string) => void;
}) {
  if (messages.length === 0) {
    return null;
  }

  return (
    <div data-chat-queue className="shrink-0 px-3 pb-1.5">
      <div className="mx-auto flex max-w-full flex-col gap-0.5">
        {messages.map((message) => (
          <div
            key={message.id}
            data-chat-queue-item
            className="group text-muted-foreground hover:bg-muted/55 grid min-h-7 grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md px-2 py-1 text-xs transition-colors"
          >
            <CornerDownRightIcon className="size-3.5" />
            <span className="truncate">{message.content}</span>
            <button
              type="button"
              aria-label={`Remove queued message: ${message.content}`}
              onClick={() => onRemoveMessage(message.id)}
              className="hover:bg-accent/20 inline-flex size-6 items-center justify-center rounded-md opacity-65 transition-opacity group-hover:opacity-100"
            >
              <Trash2Icon className="size-3.5" />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
