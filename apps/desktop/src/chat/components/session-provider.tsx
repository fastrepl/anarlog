import { Chat, useChat } from "@ai-sdk/react";
import type { ChatStatus, ChatTransport, LanguageModel, ToolSet } from "ai";
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
  buildPersistedChatMessageRow,
  getPersistedChatMessages,
  getVisibleChatMessages,
  shouldPersistFinishedMessage,
} from "~/chat/store/persisted-messages";
import { stripEphemeralToolContext } from "~/chat/tools/strip-ephemeral-tool-context";
import { useTransport } from "~/chat/transport/use-transport";
import type { HyprUIMessage } from "~/chat/types";
import * as main from "~/store/tinybase/store/main";

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
  modelOverride?: LanguageModel;
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
  modelOverride,
  extraTools,
  systemPromptOverride,
  unstyled = false,
  children,
}: ChatSessionProps) {
  const store = main.UI.useStore(main.STORE_ID);
  const chatMessagesTable = main.UI.useTable("chat_messages", main.STORE_ID);
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const [pendingManualRefs, setPendingManualRefs] = useState<ContextRef[]>([]);
  const [pendingDraftRefs, setPendingDraftRefs] = useState<ContextRef[]>([]);
  const latestChatGroupIdRef = useRef(chatGroupId);
  const latestStoreRef = useRef(store);
  const latestUserIdRef = useRef(user_id);
  const submittedChatGroupIdsRef = useRef(new Map<string, string>());

  latestChatGroupIdRef.current = chatGroupId;
  latestStoreRef.current = store;
  latestUserIdRef.current = user_id;

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
    modelOverride,
    extraTools,
    systemPromptOverride,
    store,
  );

  const persistedVisibleMessages = useMemo(
    () =>
      store && chatGroupId ? getVisibleChatMessages(store, chatGroupId) : [],
    [store, chatGroupId, chatMessagesTable],
  );

  const chat = useMemo(
    () =>
      new Chat<HyprUIMessage>({
        id: sessionId,
        messages:
          store && chatGroupId
            ? getVisibleChatMessages(store, chatGroupId)
            : [],
        transport: transport ?? unavailableChatTransport,
        onFinish: ({ message, messages, isAbort }) => {
          const currentStore = latestStoreRef.current;
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

          const persistedChatGroupId =
            submittedUserMessage && currentStore
              ? currentStore.getRow("chat_messages", submittedUserMessage.id)
                  ?.chat_group_id
              : undefined;
          const targetChatGroupId =
            (typeof persistedChatGroupId === "string" && persistedChatGroupId
              ? persistedChatGroupId
              : undefined) ??
            submittedChatGroupId ??
            latestChatGroupIdRef.current;

          if (
            isAbort ||
            !targetChatGroupId ||
            !currentStore ||
            !currentUserId
          ) {
            return;
          }

          const sanitizedParts = stripEphemeralToolContext(message.parts);
          const sanitizedMessage =
            sanitizedParts === message.parts
              ? message
              : { ...message, parts: sanitizedParts };
          if (!shouldPersistFinishedMessage(sanitizedMessage)) {
            currentStore.delRow("chat_messages", sanitizedMessage.id);
            return;
          }
          currentStore.setRow(
            "chat_messages",
            sanitizedMessage.id,
            buildPersistedChatMessageRow({
              message: sanitizedMessage,
              chatGroupId: targetChatGroupId,
              userId: currentUserId,
              status: "ready",
              existingRow: currentStore.getRow(
                "chat_messages",
                sanitizedMessage.id,
              ),
            }),
          );
        },
      }),
    [sessionId, store, transport],
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
    if (!store || !chatGroupId) return;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role !== "assistant") {
        continue;
      }

      store.delRow("chat_messages", messages[i].id);
      break;
    }
    void chatRegenerate();
  }, [store, chatGroupId, messages, chatRegenerate]);

  const setMessages = useCallback(
    (next: HyprUIMessage[] | ((prev: HyprUIMessage[]) => HyprUIMessage[])) => {
      chatSetMessages(next);
      if (!store || !chatGroupId) return;
      const resolved = typeof next === "function" ? next(messages) : next;
      const nextIds = new Set(resolved.map((m) => m.id));
      store.transaction(() => {
        getPersistedChatMessages(store, chatGroupId).forEach(({ id }) => {
          if (!nextIds.has(id)) store.delRow("chat_messages", id);
        });
      });
    },
    [chatGroupId, messages, chatSetMessages, store],
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
    store,
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
