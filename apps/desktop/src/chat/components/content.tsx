import { ArrowElbowDownRight, Trash } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ChatStatus } from "ai";
import { useCallback, useEffect, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

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
import type { ChatMessageSender, AnlgUIMessage } from "~/chat/types";
import { id } from "~/shared/utils";

type QueuedChatMessage = {
  id: string;
  content: string;
  parts: AnlgUIMessage["parts"];
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
  messages: AnlgUIMessage[];
  sendMessage: ChatMessageSender;
  regenerate: () => void;
  stop: () => void;
  status: ChatStatus;
  error?: Error;
  model: ReturnType<typeof useLanguageModel>;
  handleSendMessage: (
    content: string,
    parts: AnlgUIMessage["parts"],
    sendMessage: ChatMessageSender,
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
      parts: AnlgUIMessage["parts"],
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
      data-chat-content
      data-chat-layout={layout}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      {...stylex.props([
        styles.root,
        isFloating ? styles.floatingRoot : styles.rightPanelRoot,
      ])}
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
            layout={layout}
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
    <div data-chat-queue {...stylex.props(styles.queue)}>
      <div {...stylex.props(styles.queueList)}>
        {messages.map((message) => (
          <div
            key={message.id}
            data-chat-queue-item
            {...stylex.props(styles.queueItem)}
          >
            <ArrowElbowDownRight {...stylex.props(styles.queueIcon)} />
            <span {...stylex.props(styles.truncate)}>{message.content}</span>
            <button
              type="button"
              aria-label={`Remove queued message: ${message.content}`}
              onClick={() => onRemoveMessage(message.id)}
              {...stylex.props(styles.removeButton)}
            >
              <Trash {...stylex.props(styles.queueIcon)} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    flexDirection: "column",
    minHeight: 0,
    overflow: "hidden",
  },
  floatingRoot: {
    maxHeight: "100%",
  },
  rightPanelRoot: {
    flex: "1",
  },
  queue: {
    flexShrink: 0,
    paddingBottom: "0.375rem",
    paddingInline: "0.75rem",
  },
  queueList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
    marginInline: "auto",
    maxWidth: "100%",
  },
  queueItem: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.muted} 55%, transparent)`,
    },
    borderRadius: radii.md,
    color: colors.mutedForeground,
    display: "grid",
    fontSize: "0.75rem",
    gap: "0.5rem",
    gridTemplateColumns: "1rem minmax(0, 1fr) auto",
    lineHeight: "1rem",
    minHeight: "1.75rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  queueIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  removeButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.accent} 20%, transparent)`,
    },
    borderRadius: radii.md,
    display: "inline-flex",
    height: "1.5rem",
    justifyContent: "center",
    opacity: {
      default: 0.65,
      ":is([data-chat-queue-item]:hover *)": 1,
    },
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.5rem",
  },
});
