import { useChat } from "@ai-sdk/react";
import type { ChatStatus } from "ai";
import type { LanguageModel, ToolSet } from "ai";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { ContextRef } from "~/chat/context/entities";
import {
  type DisplayEntity,
  useChatContextPipeline,
} from "~/chat/context/use-chat-context-pipeline";
import { useCreateChatMessage } from "~/chat/store/useCreateChatMessage";
import { stripEphemeralToolContext } from "~/chat/tools/strip-ephemeral-tool-context";
import { useTransport } from "~/chat/transport/use-transport";
import type { HyprUIMessage } from "~/chat/types";
import { id } from "~/shared/utils";
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

  const [pendingManualRefs, setPendingManualRefs] = useState<ContextRef[]>([]);

  const onAddContextEntity = useCallback((ref: ContextRef) => {
    setPendingManualRefs((prev) =>
      prev.some((r) => r.key === ref.key) ? prev : [...prev, ref],
    );
  }, []);

  const onRemoveContextEntity = useCallback((key: string) => {
    setPendingManualRefs((prev) => prev.filter((r) => r.key !== key));
  }, []);

  // Clear pending manual refs when the conversation changes.
  useEffect(() => {
    setPendingManualRefs([]);
  }, [sessionId, chatGroupId]);

  const { transport, isSystemPromptReady } = useTransport(
    modelOverride,
    extraTools,
    systemPromptOverride,
    store,
  );
  const createChatMessage = useCreateChatMessage();

  const messageIds = main.UI.useSliceRowIds(
    main.INDEXES.chatMessagesByGroup,
    chatGroupId ?? "",
    main.STORE_ID,
  );

  const initialMessages = useMemo((): HyprUIMessage[] => {
    if (!store || !chatGroupId) {
      return [];
    }

    const loaded: HyprUIMessage[] = [];
    for (const messageId of messageIds) {
      const row = store.getRow("chat_messages", messageId);
      if (row) {
        let parsedParts: HyprUIMessage["parts"] = [];
        let parsedMetadata: Record<string, unknown> = {};
        try {
          parsedParts = JSON.parse(row.parts ?? "[]");
        } catch {}
        try {
          parsedMetadata = JSON.parse(row.metadata ?? "{}");
        } catch {}
        loaded.push({
          id: messageId as string,
          role: row.role as "user" | "assistant",
          parts: parsedParts,
          metadata: parsedMetadata,
        });
      }
    }
    return loaded;
  }, [store, messageIds, chatGroupId]);

  const {
    messages,
    setMessages,
    sendMessage: rawSendMessage,
    regenerate,
    stop,
    status,
    error,
  } = useChat({
    id: sessionId,
    messages: initialMessages,
    generateId: () => id(),
    transport: transport ?? undefined,
    onError: console.error,
  });

  useEffect(() => {
    if (!chatGroupId || !store) {
      return;
    }

    const assistantMessages = messages.filter(
      (message) => message.role === "assistant",
    );
    const assistantMessageIds = new Set(assistantMessages.map((m) => m.id));

    for (const messageId of messageIds) {
      if (assistantMessageIds.has(messageId)) {
        continue;
      }
      const row = store.getRow("chat_messages", messageId);
      if (row?.role === "assistant") {
        store.delRow("chat_messages", messageId);
      }
    }

    if (status === "ready") {
      for (const message of assistantMessages) {
        if (store.hasRow("chat_messages", message.id)) {
          continue;
        }
        const sanitizedParts = stripEphemeralToolContext(message.parts);

        const content = sanitizedParts
          .filter(
            (p): p is Extract<typeof p, { type: "text" }> => p.type === "text",
          )
          .map((p) => p.text)
          .join("");

        createChatMessage({
          id: message.id,
          chat_group_id: chatGroupId,
          content,
          role: "assistant",
          parts: sanitizedParts,
          metadata: message.metadata,
        });
      }
    }
  }, [chatGroupId, messages, status, store, createChatMessage, messageIds]);

  useEffect(() => {
    if (status !== "ready") {
      return;
    }

    setMessages((prev) => {
      let changed = false;
      const next = prev.map((message) => {
        if (message.role !== "assistant") {
          return message;
        }

        const sanitizedParts = stripEphemeralToolContext(message.parts);
        if (sanitizedParts === message.parts) {
          return message;
        }

        changed = true;
        return {
          ...message,
          parts: sanitizedParts,
        };
      });

      return changed ? next : prev;
    });
  }, [status, setMessages]);

  // Clear pending manual refs once a user message is committed to history.
  const prevUserMsgCountRef = useRef(0);
  useEffect(() => {
    const count = messages.filter((m) => m.role === "user").length;
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
        sendMessage: rawSendMessage,
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
