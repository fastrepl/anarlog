import { type ReactNode, useCallback } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatSession, type ChatSessionRenderProps } from "./session-provider";
import { ChatToolbarControls } from "./toolbar-controls";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { chatFloatingPanelClassNames } from "~/chat/surface";
import { useShell } from "~/contexts/shell";
import * as main from "~/store/tinybase/store/main";

export function ChatView({
  layout = "floating",
  onOpenFloating,
  onOpenRightPanel,
}: {
  layout?: "floating" | "right-panel";
  onOpenFloating?: () => void;
  onOpenRightPanel?: () => void;
}) {
  return (
    <ChatSessionHost>
      {(sessionProps) => (
        <ChatPanelFrame
          layout={layout}
          onOpenFloating={onOpenFloating}
          onOpenRightPanel={onOpenRightPanel}
          sessionProps={sessionProps}
        />
      )}
    </ChatSessionHost>
  );
}

export function ChatSessionHost({
  children,
}: {
  children: (sessionProps: ChatSessionRenderProps | null) => ReactNode;
}) {
  const { chat } = useShell();
  const { groupId, sessionId } = chat;
  const { currentSessionId } = useSessionTab();
  const { user_id } = main.UI.useValues(main.STORE_ID);

  if (!user_id) {
    return <>{children(null)}</>;
  }

  return (
    <ChatSession
      sessionId={sessionId}
      chatGroupId={groupId}
      currentSessionId={currentSessionId}
      unstyled
    >
      {children}
    </ChatSession>
  );
}

export function ChatPanelFrame({
  layout = "floating",
  onDraftContentChange,
  onOpenFloating,
  onOpenRightPanel,
  sessionProps,
}: {
  layout?: "floating" | "right-panel";
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  onOpenFloating?: () => void;
  onOpenRightPanel?: () => void;
  sessionProps: ChatSessionRenderProps | null;
}) {
  const { chat } = useShell();
  const { groupId, setGroupId } = chat;
  const { panelClassName, toolbarSurface } = useChatAppearance();
  const isFloating = layout === "floating";
  const model = useLanguageModel("chat");

  const handleGroupCreated = useCallback(
    (newGroupId: string) => {
      setGroupId(newGroupId);
    },
    [setGroupId],
  );

  const { handleSendMessage } = useChatActions({
    groupId,
    onGroupCreated: handleGroupCreated,
  });

  return (
    <div
      className={cn([
        "flex min-h-0 flex-col overflow-hidden",
        isFloating ? "max-h-full" : "h-full",
        isFloating ? chatFloatingPanelClassNames() : panelClassName,
      ])}
    >
      <div
        className={cn([
          "flex shrink-0 items-center pr-0 pl-0",
          isFloating ? "h-11" : "h-12",
        ])}
      >
        <ChatToolbarControls
          currentChatGroupId={groupId}
          layout={layout}
          onClose={() => chat.sendEvent({ type: "CLOSE" })}
          onNewChat={chat.startNewChat}
          onOpenFloating={onOpenFloating}
          onOpenRightPanel={onOpenRightPanel}
          onSelectChat={chat.selectChat}
          surface={toolbarSurface}
        />
      </div>
      {sessionProps && (
        <ChatContent
          {...sessionProps}
          layout={layout}
          onDraftContentChange={onDraftContentChange}
          model={model}
          handleSendMessage={handleSendMessage}
        >
          <ChatBody
            messages={sessionProps.messages}
            status={sessionProps.status}
            error={sessionProps.error}
            onReload={sessionProps.regenerate}
            isModelConfigured={!!model}
            hasContext={sessionProps.contextEntities.length > 0}
            onSendMessage={(content, parts) => {
              handleSendMessage(
                content,
                parts,
                sessionProps.sendMessage,
                sessionProps.pendingRefs,
              );
            }}
          />
        </ChatContent>
      )}
    </div>
  );
}
