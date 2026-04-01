import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";

import { cn } from "@hypr/utils";

import { useFeedbackLanguageModel } from "~/ai/hooks";
import { useAuth } from "~/auth";
import { useChatwootEvents } from "~/chat/chatwoot/useChatwootEvents";
import { useChatwootPersistence } from "~/chat/chatwoot/useChatwootPersistence";
import { ChatBody } from "~/chat/components/body";
import { ChatContent } from "~/chat/components/content";
import { ChatSession } from "~/chat/components/session-provider";
import { type ContextEntity, dedupeByKey } from "~/chat/context/entities";
import { useSupportMCP } from "~/chat/mcp/useSupportMCP";
import {
  useChatActions,
  useStableSessionId,
} from "~/chat/store/use-chat-actions";
import type { HyprUIMessage } from "~/chat/types";
import { ElicitationProvider } from "~/contexts/elicitation";
import { StandardTabWrapper } from "~/shared/main";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import type { Tab } from "~/store/zustand/tabs";
import { useTabs } from "~/store/zustand/tabs";

export function TabContentChat({
  tab,
}: {
  tab: Extract<Tab, { type: "chat_support" }>;
}) {
  return (
    <StandardTabWrapper>
      <SupportChatTabView tab={tab} />
    </StandardTabWrapper>
  );
}

function SupportChatTabView({
  tab,
}: {
  tab: Extract<Tab, { type: "chat_support" }>;
}) {
  const groupId = tab.state.groupId ?? undefined;
  const updateChatSupportTabState = useTabs(
    (state) => state.updateChatSupportTabState,
  );
  const { session } = useAuth();

  const stableSessionId = useStableSessionId(groupId);
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const feedbackModel = useFeedbackLanguageModel();
  const {
    tools: mcpTools,
    systemPrompt,
    contextEntities: supportContextEntities,
    pendingElicitation,
    respondToElicitation,
    isReady,
  } = useSupportMCP(true, session?.access_token);

  const chatwoot = useChatwootPersistence(user_id, {
    email: session?.user.email ?? undefined,
    name: session?.user.user_metadata?.full_name as string | undefined,
  });

  const mcpToolCount = Object.keys(mcpTools).length;

  const onGroupCreated = useCallback(
    (newGroupId: string) =>
      updateChatSupportTabState(tab, {
        ...tab.state,
        groupId: newGroupId,
        initialMessage: null,
      }),
    [updateChatSupportTabState, tab],
  );

  const { handleSendMessage } = useChatActions({
    groupId,
    onGroupCreated,
  });

  if (!isReady) {
    return (
      <div className="flex h-full flex-col bg-sky-50/40">
        <div className="flex flex-1 items-center justify-center">
          <div className="flex items-center gap-2 text-sm text-neutral-500">
            <Loader2 className="size-4 animate-spin" />
            <span>Preparing support chat...</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={cn(["flex h-full flex-col", "bg-sky-50/40"])}>
      {user_id && (
        <ChatSession
          key={`${stableSessionId}-${mcpToolCount}`}
          sessionId={stableSessionId}
          chatGroupId={groupId}
          modelOverride={feedbackModel}
          extraTools={mcpTools}
          systemPromptOverride={systemPrompt}
        >
          {(sessionProps) => (
            <SupportChatTabInner
              tab={tab}
              sessionProps={sessionProps}
              feedbackModel={feedbackModel}
              handleSendMessage={handleSendMessage}
              updateChatSupportTabState={updateChatSupportTabState}
              supportContextEntities={supportContextEntities}
              pendingElicitation={pendingElicitation}
              respondToElicitation={respondToElicitation}
              chatwoot={chatwoot}
            />
          )}
        </ChatSession>
      )}
    </div>
  );
}

function SupportChatTabInner({
  tab,
  sessionProps,
  feedbackModel,
  handleSendMessage: rawHandleSendMessage,
  updateChatSupportTabState,
  supportContextEntities,
  pendingElicitation,
  respondToElicitation,
  chatwoot,
}: {
  tab: Extract<Tab, { type: "chat_support" }>;
  sessionProps: {
    sessionId: string;
    messages: HyprUIMessage[];
    setMessages: (
      msgs: HyprUIMessage[] | ((prev: HyprUIMessage[]) => HyprUIMessage[]),
    ) => void;
    sendMessage: (message: HyprUIMessage) => void;
    regenerate: () => void;
    stop: () => void;
    status: "submitted" | "streaming" | "ready" | "error";
    error?: Error;
    contextEntities: import("~/chat/context/use-chat-context-pipeline").DisplayEntity[];
    pendingRefs: import("~/chat/context/entities").ContextRef[];
    onRemoveContextEntity: (key: string) => void;
    isSystemPromptReady: boolean;
  };
  feedbackModel: ReturnType<typeof useFeedbackLanguageModel>;
  handleSendMessage: (
    content: string,
    parts: HyprUIMessage["parts"],
    sendMessage: (message: HyprUIMessage) => void,
  ) => void;
  updateChatSupportTabState: (
    tab: Extract<Tab, { type: "chat_support" }>,
    state: Extract<Tab, { type: "chat_support" }>["state"],
  ) => void;
  supportContextEntities: ContextEntity[];
  pendingElicitation?: { message: string } | null;
  respondToElicitation?: (approved: boolean) => void;
  chatwoot: ReturnType<typeof useChatwootPersistence>;
}) {
  const {
    messages,
    sendMessage,
    regenerate,
    stop,
    status,
    error,
    contextEntities,
    pendingRefs,
    onRemoveContextEntity,
    isSystemPromptReady,
  } = sessionProps;
  const sentRef = useRef(false);
  const chatwootConvStartedRef = useRef(false);
  const lastPersistedMsgCountRef = useRef(0);

  // Start a chatwoot conversation on first user message
  const handleSendMessage = useCallback(
    (
      content: string,
      parts: HyprUIMessage["parts"],
      sendMsg: (message: HyprUIMessage) => void,
    ) => {
      if (chatwoot.isReady) {
        if (!chatwootConvStartedRef.current) {
          chatwootConvStartedRef.current = true;
          chatwoot.startConversation();
        }
        chatwoot.persistMessage(content, "incoming");
      }
      rawHandleSendMessage(content, parts, sendMsg);
    },
    [
      rawHandleSendMessage,
      chatwoot.isReady,
      chatwoot.startConversation,
      chatwoot.persistMessage,
    ],
  );

  // Persist AI responses to chatwoot when they finish streaming
  const prevStatusRef = useRef(status);
  useEffect(() => {
    const wasStreaming =
      prevStatusRef.current === "streaming" ||
      prevStatusRef.current === "submitted";
    prevStatusRef.current = status;

    if (wasStreaming && status === "ready" && chatwoot.conversationId != null) {
      const assistantMsgs = messages.filter((m) => m.role === "assistant");
      if (assistantMsgs.length > lastPersistedMsgCountRef.current) {
        const latest = assistantMsgs[assistantMsgs.length - 1];
        if ((latest.metadata as Record<string, unknown>)?.chatwootAgent) {
          lastPersistedMsgCountRef.current = assistantMsgs.length;
          return;
        }
        const textContent = latest.parts
          .filter(
            (p): p is Extract<typeof p, { type: "text" }> => p.type === "text",
          )
          .map((p) => p.text)
          .join("");
        if (textContent) {
          chatwoot.persistMessage(textContent, "outgoing");
        }
        lastPersistedMsgCountRef.current = assistantMsgs.length;
      }
    }
  }, [status, messages, chatwoot.conversationId, chatwoot.persistMessage]);

  // Listen for human agent replies from chatwoot and inject into chat
  const { setMessages } = sessionProps;
  useChatwootEvents({
    pubsubToken: chatwoot.pubsubToken,
    conversationId: chatwoot.conversationId,
    onAgentMessage: useCallback(
      (content: string, senderName: string) => {
        const agentMessage: HyprUIMessage = {
          id: id(),
          role: "assistant",
          parts: [{ type: "text", text: `**${senderName}:** ${content}` }],
          metadata: {
            createdAt: Date.now(),
            chatwootAgent: true,
          } as HyprUIMessage["metadata"],
        };
        setMessages((prev) => [...prev, agentMessage]);
      },
      [setMessages],
    ),
  });

  useEffect(() => {
    const initialMessage = tab.state.initialMessage;
    if (
      !initialMessage ||
      sentRef.current ||
      !feedbackModel ||
      status !== "ready" ||
      !isSystemPromptReady
    ) {
      return;
    }

    sentRef.current = true;
    handleSendMessage(
      initialMessage,
      [{ type: "text", text: initialMessage }],
      sendMessage,
    );
    updateChatSupportTabState(tab, {
      ...tab.state,
      initialMessage: null,
    });
  }, [
    tab,
    feedbackModel,
    status,
    isSystemPromptReady,
    handleSendMessage,
    sendMessage,
    updateChatSupportTabState,
  ]);

  const mergedContextEntities = dedupeByKey([
    contextEntities,
    supportContextEntities.map((e) => ({ ...e, pending: false as const })),
  ]);

  return (
    <ChatContent
      sessionId={sessionProps.sessionId}
      messages={messages}
      sendMessage={sendMessage}
      regenerate={regenerate}
      stop={stop}
      status={status}
      error={error}
      model={feedbackModel}
      handleSendMessage={handleSendMessage}
      contextEntities={mergedContextEntities}
      pendingRefs={pendingRefs}
      onRemoveContextEntity={onRemoveContextEntity}
      isSystemPromptReady={isSystemPromptReady}
      mcpIndicator={{ type: "support" }}
    >
      <ElicitationProvider
        pending={pendingElicitation ?? null}
        respond={respondToElicitation ?? null}
      >
        <ChatBody
          messages={messages}
          status={status}
          error={error}
          onReload={regenerate}
          isModelConfigured={!!feedbackModel}
        />
      </ElicitationProvider>
    </ChatContent>
  );
}
