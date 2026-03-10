import { useChat } from "@ai-sdk/react";
import type { ChatStatus, LanguageModel, ToolSet } from "ai";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import type { ContextRef } from "~/chat/context/entities";
import {
  type DisplayEntity,
  useChatContextPipeline,
} from "~/chat/context/use-chat-context-pipeline";
import {
  buildPersistedChatMessageRow,
  getPersistedChatMessages,
  getVisibleChatMessages,
} from "~/chat/store/persisted-messages";
import { stripEphemeralToolContext } from "~/chat/tools/strip-ephemeral-tool-context";
import { useTransport } from "~/chat/transport/use-transport";
import type { HyprUIMessage } from "~/chat/types";
import * as main from "~/store/tinybase/store/main";

interface ChatSessionProps {
  sessionId: string;
  chatGroupId?: string;
  currentSessionId?: string;
  modelOverride?: LanguageModel;
  extraTools?: ToolSet;
  systemPromptOverride?: string;
  children: (props: {
    sessionId: string;
    messages: HyprUIMessage[];
    setMessages: (
      msgs: HyprUIMessage[] | ((prev: HyprUIMessage[]) => HyprUIMessage[]),
    ) => void;
    sendMessage: (message: HyprUIMessage) => void;
    regenerate: () => void;
    stop: () => void;
    status: ChatStatus;
    error?: Error;
    contextEntities: DisplayEntity[];
    pendingRefs: ContextRef[];
    onRemoveContextEntity: (key: string) => void;
    onAddContextEntity: (ref: ContextRef) => void;
    isSystemPromptReady: boolean;
  }) => ReactNode;
}

export function ChatSession({
  sessionId,
  chatGroupId,
  currentSessionId,
  modelOverride,
  extraTools,
  systemPromptOverride,
  children,
}: ChatSessionProps) {
  const store = main.UI.useStore(main.STORE_ID);
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const [pendingManualRefs, setPendingManualRefs] = useState<ContextRef[]>([]);

  const onAddContextEntity = useCallback((ref: ContextRef) => {
    setPendingManualRefs((prev) =>
      prev.some((r) => r.key === ref.key) ? prev : [...prev, ref],
    );
  }, []);

  const onRemoveContextEntity = useCallback((key: string) => {
    setPendingManualRefs((prev) => prev.filter((r) => r.key !== key));
  }, []);

  useEffect(() => {
    setPendingManualRefs([]);
  }, [sessionId, chatGroupId]);

  const { transport, isSystemPromptReady } = useTransport(
    modelOverride,
    extraTools,
    systemPromptOverride,
    store,
  );

  const {
    messages,
    sendMessage: chatSendMessage,
    regenerate: chatRegenerate,
    stop,
    status,
    error,
    setMessages: chatSetMessages,
  } = useChat<HyprUIMessage>({
    id: sessionId,
    messages:
      store && chatGroupId ? getVisibleChatMessages(store, chatGroupId) : [],
    transport: transport ?? undefined,
    onFinish: ({ message, isAbort }) => {
      if (isAbort || !chatGroupId || !store || !user_id) return;
      const sanitizedParts = stripEphemeralToolContext(message.parts);
      const sanitizedMessage =
        sanitizedParts === message.parts
          ? message
          : { ...message, parts: sanitizedParts };
      store.setRow(
        "chat_messages",
        sanitizedMessage.id,
        buildPersistedChatMessageRow({
          message: sanitizedMessage,
          chatGroupId,
          userId: user_id,
          status: "ready",
          existingRow: store.getRow("chat_messages", sanitizedMessage.id),
        }),
      );
    },
  });

  const sendMessage = useCallback(
    (message: HyprUIMessage) => {
      // HyprUIMessage is structurally compatible with CreateUIMessage<HyprUIMessage>:
      // no `text`/`files` so the SDK takes the `else` branch and uses message.id as the message id.
      void chatSendMessage(message as Parameters<typeof chatSendMessage>[0]);
    },
    [chatSendMessage],
  );

  const regenerate = useCallback(() => {
    if (!store || !chatGroupId) return;
    const last = [...getPersistedChatMessages(store, chatGroupId)]
      .reverse()
      .find((m) => m.message.role === "assistant");
    if (last) store.delRow("chat_messages", last.id);
    void chatRegenerate();
  }, [store, chatGroupId, chatRegenerate]);

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
    }
    prevUserMsgCountRef.current = count;
  }, [messages]);

  const { contextEntities, pendingRefs } = useChatContextPipeline({
    messages,
    currentSessionId,
    pendingManualRefs,
    store,
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {children({
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
        isSystemPromptReady,
      })}
    </div>
  );
}
