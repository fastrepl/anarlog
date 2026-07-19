import { Chat, useChat } from "@ai-sdk/react";
import type { ChatStatus, ChatTransport, ToolSet } from "ai";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { dedupeByKey, type ContextRef } from "~/chat/context/entities";
import {
  type DisplayEntity,
  useChatContextPipeline,
} from "~/chat/context/use-chat-context-pipeline";
import {
  buildPersistedChatMessage,
  getVisibleChatMessages,
  shouldPersistFinishedMessage,
} from "~/chat/store/persisted-messages";
import {
  deleteChatMessage,
  deleteChatMessagesExcept,
  getChatMessageGroupId,
  upsertChatMessage,
  usePersistedChatMessages,
} from "~/chat/store/queries";
import { stripEphemeralToolContext } from "~/chat/tools/strip-ephemeral-tool-context";
import { useTransport } from "~/chat/transport/use-transport";
import type { HyprUIMessage } from "~/chat/types";
import { useOwnerUserId } from "~/shared/owner-user";

export type ChatSessionRenderProps = {
  sessionId: string;
  messages: HyprUIMessage[];
  setMessages: (
    msgs: HyprUIMessage[] | ((prev: HyprUIMessage[]) => HyprUIMessage[]),
  ) => void;
  sendMessage: (
    message: HyprUIMessage,
    options?: { chatGroupId?: string },
  ) => void;
  regenerate: () => void;
  stop: () => void;
  status: ChatStatus;
  error?: Error;
  contextEntities: DisplayEntity[];
  pendingRefs: ContextRef[];
  onRemoveContextEntity: (key: string) => void;
  onAddContextEntity: (ref: ContextRef) => void;
  onDraftContextRefsChange: (refs: ContextRef[]) => void;
  isSystemPromptReady: boolean;
};

interface ChatSessionProps {
  sessionId: string;
  chatGroupId?: string;
  currentSessionId?: string;
  extraTools?: ToolSet;
  systemPromptOverride?: string;
  unstyled?: boolean;
  children: (props: ChatSessionRenderProps) => ReactNode;
}

function areMessagesEqual(a: HyprUIMessage[], b: HyprUIMessage[]) {
  if (a.length !== b.length) {
    return false;
  }

  return a.every((message, index) => {
    const other = b[index];
    return (
      message.id === other?.id &&
      message.role === other.role &&
      JSON.stringify(message.parts) === JSON.stringify(other.parts) &&
      JSON.stringify(message.metadata ?? {}) ===
        JSON.stringify(other.metadata ?? {})
    );
  });
}

export function ChatSession({
  sessionId,
  chatGroupId,
  currentSessionId,
  extraTools,
  systemPromptOverride,
  unstyled = false,
  children,
}: ChatSessionProps) {
  const ownerUserId = useOwnerUserId();
  const persistedMessages = usePersistedChatMessages(chatGroupId);

  const [pendingManualRefs, setPendingManualRefs] = useState<ContextRef[]>([]);
  const [pendingDraftRefs, setPendingDraftRefs] = useState<ContextRef[]>([]);
  const latestChatGroupIdRef = useRef(chatGroupId);
  const latestUserIdRef = useRef(ownerUserId);
  const initialMessagesRef = useRef<HyprUIMessage[]>([]);
  const submittedChatGroupIdsRef = useRef(new Map<string, string>());

  latestChatGroupIdRef.current = chatGroupId;
  latestUserIdRef.current = ownerUserId;

  const onAddContextEntity = useCallback((ref: ContextRef) => {
    setPendingManualRefs((prev) =>
      prev.some((r) => r.key === ref.key) ? prev : [...prev, ref],
    );
  }, []);

  const onRemoveContextEntity = useCallback((key: string) => {
    setPendingManualRefs((prev) => prev.filter((r) => r.key !== key));
    setPendingDraftRefs((prev) => prev.filter((r) => r.key !== key));
  }, []);

  const onDraftContextRefsChange = useCallback((refs: ContextRef[]) => {
    setPendingDraftRefs(refs);
  }, []);

  useEffect(() => {
    setPendingManualRefs([]);
    setPendingDraftRefs([]);
  }, [sessionId, chatGroupId]);

  const { transport, isSystemPromptReady } = useTransport(
    extraTools,
    systemPromptOverride,
    ownerUserId || undefined,
  );

  const persistedVisibleMessages = useMemo(
    () => getVisibleChatMessages(persistedMessages),
    [persistedMessages],
  );
  initialMessagesRef.current = persistedVisibleMessages;

  const chat = useMemo(
    () =>
      new Chat<HyprUIMessage>({
        id: sessionId,
        messages: initialMessagesRef.current,
        transport: transport ?? unavailableChatTransport,
        onFinish: ({ message, messages, isAbort }) => {
          const currentUserId = latestUserIdRef.current;
          const messageIndex = messages.findIndex((m) => m.id === message.id);
          const lastMessageIndex =
            messageIndex === -1 ? messages.length - 1 : messageIndex - 1;
          let submittedUserMessage: HyprUIMessage | undefined;
          for (let i = lastMessageIndex; i >= 0; i--) {
            if (messages[i].role === "user") {
              submittedUserMessage = messages[i];
              break;
            }
          }
          const submittedChatGroupId = submittedUserMessage
            ? submittedChatGroupIdsRef.current.get(submittedUserMessage.id)
            : undefined;
          if (submittedUserMessage) {
            submittedChatGroupIdsRef.current.delete(submittedUserMessage.id);
          }

          if (isAbort || !currentUserId) {
            return;
          }

          void (async () => {
            let persistedChatGroupId: string | null = null;
            if (!submittedChatGroupId && submittedUserMessage) {
              try {
                persistedChatGroupId = await getChatMessageGroupId(
                  submittedUserMessage.id,
                );
              } catch (error) {
                console.error(
                  "Failed to resolve the persisted chat message group",
                  error,
                );
              }
            }
            const targetChatGroupId =
              submittedChatGroupId ??
              persistedChatGroupId ??
              latestChatGroupIdRef.current;
            if (!targetChatGroupId) {
              return;
            }

            const sanitizedParts = stripEphemeralToolContext(message.parts);
            const sanitizedMessage =
              sanitizedParts === message.parts
                ? message
                : { ...message, parts: sanitizedParts };
            if (!shouldPersistFinishedMessage(sanitizedMessage)) {
              await deleteChatMessage(targetChatGroupId, sanitizedMessage.id);
              return;
            }

            await upsertChatMessage(
              buildPersistedChatMessage({
                message: sanitizedMessage,
                chatGroupId: targetChatGroupId,
                ownerUserId: currentUserId,
                status: "ready",
              }),
            );
          })().catch((error) => {
            console.error("Failed to persist finished chat message", error);
          });
        },
      }),
    [sessionId, transport],
  );

  const {
    messages,
    sendMessage: chatSendMessage,
    regenerate: chatRegenerate,
    stop,
    status,
    error,
    setMessages: chatSetMessages,
  } = useChat<HyprUIMessage>({ chat });

  useEffect(() => {
    if (
      status !== "ready" ||
      !chatGroupId ||
      areMessagesEqual(messages, persistedVisibleMessages)
    ) {
      return;
    }

    chatSetMessages(persistedVisibleMessages);
  }, [
    chatGroupId,
    messages,
    persistedVisibleMessages,
    status,
    chatSetMessages,
  ]);

  const sendMessage = useCallback(
    (message: HyprUIMessage, options?: { chatGroupId?: string }) => {
      const targetChatGroupId =
        options?.chatGroupId ?? latestChatGroupIdRef.current;
      if (targetChatGroupId) {
        submittedChatGroupIdsRef.current.set(message.id, targetChatGroupId);
      }
      // HyprUIMessage is structurally compatible with CreateUIMessage<HyprUIMessage>:
      // no `text`/`files` so the SDK takes the `else` branch and uses message.id as the message id.
      void chatSendMessage(message as Parameters<typeof chatSendMessage>[0]);
    },
    [chatSendMessage],
  );

  const regenerate = useCallback(() => {
    if (!chatGroupId) return;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role !== "assistant") {
        continue;
      }

      void deleteChatMessage(chatGroupId, messages[i].id)
        .then(() => chatRegenerate())
        .catch((error) => {
          console.error("Failed to remove regenerated chat message", error);
        });
      return;
    }
    void chatRegenerate();
  }, [chatGroupId, messages, chatRegenerate]);

  const setMessages = useCallback(
    (next: HyprUIMessage[] | ((prev: HyprUIMessage[]) => HyprUIMessage[])) => {
      chatSetMessages(next);
      if (!chatGroupId) return;
      const resolved = typeof next === "function" ? next(messages) : next;
      void deleteChatMessagesExcept(
        chatGroupId,
        resolved.map((message) => message.id),
      ).catch((error) => {
        console.error("Failed to reconcile persisted chat messages", error);
      });
    },
    [chatGroupId, messages, chatSetMessages],
  );

  const prevUserMsgCountRef = useRef(0);
  useEffect(() => {
    const count = messages.filter((message) => message.role === "user").length;
    if (count > prevUserMsgCountRef.current) {
      setPendingManualRefs([]);
      setPendingDraftRefs([]);
    }
    prevUserMsgCountRef.current = count;
  }, [messages]);

  const pendingMessageRefs = useMemo(
    () => dedupeByKey([pendingManualRefs, pendingDraftRefs]),
    [pendingManualRefs, pendingDraftRefs],
  );

  const { contextEntities, pendingRefs } = useChatContextPipeline({
    messages,
    currentSessionId,
    pendingManualRefs: pendingMessageRefs,
  });

  const content = children({
    sessionId,
    messages,
    setMessages,
    sendMessage,
    regenerate,
    stop,
    status,
    error,
    contextEntities,
    pendingRefs,
    onRemoveContextEntity,
    onAddContextEntity,
    onDraftContextRefsChange,
    isSystemPromptReady,
  });

  if (unstyled) {
    return content;
  }

  return <div className="flex min-h-0 flex-1 flex-col">{content}</div>;
}

const unavailableChatTransport: ChatTransport<HyprUIMessage> = {
  sendMessages: async () => {
    throw new Error("Chat model is not ready");
  },
  reconnectToStream: async () => null,
};
