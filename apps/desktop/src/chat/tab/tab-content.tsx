import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";

import { cn } from "@hypr/utils";

import { useFeedbackLanguageModel } from "~/ai/hooks";
import { useAuth } from "~/auth";
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
  handleSendMessage,
  updateChatSupportTabState,
  supportContextEntities,
  pendingElicitation,
  respondToElicitation,
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
