import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatSession } from "./session-provider";
import { ChatToolbarControls } from "./toolbar-controls";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import { useChatActions } from "~/chat/store/use-chat-actions";
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
  const { chat } = useShell();
  const { groupId, sessionId, setGroupId } = chat;
  const isFloating = layout === "floating";

  const { currentSessionId } = useSessionTab();

  const model = useLanguageModel("chat");
  const { user_id } = main.UI.useValues(main.STORE_ID);

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
        "flex h-full min-h-0 flex-col overflow-hidden",
        "bg-stone-800 text-white",
      ])}
    >
      <div
        className={cn([
          "flex shrink-0 items-center pr-0 pl-0",
          isFloating ? "h-11" : "h-12",
          "border-b border-stone-700/80",
        ])}
      >
        <ChatToolbarControls
          currentChatGroupId={groupId}
          layout={layout}
          onNewChat={chat.startNewChat}
          onOpenFloating={onOpenFloating}
          onOpenRightPanel={onOpenRightPanel}
          onSelectChat={chat.selectChat}
          surface="dark"
        />
      </div>
      {user_id && (
        <ChatSession
          key={sessionId}
          sessionId={sessionId}
          chatGroupId={groupId}
          currentSessionId={currentSessionId}
        >
          {(sessionProps) => (
            <ChatContent
              {...sessionProps}
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
        </ChatSession>
      )}
    </div>
  );
}
