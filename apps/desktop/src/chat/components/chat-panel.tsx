import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatHeader } from "./header";
import { ChatSession } from "./session-provider";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import { useChatActions } from "~/chat/hooks/use-chat-actions";
import { useShell } from "~/contexts/shell";
import { useCurrentUserId } from "~/session/hooks/storage";

export function ChatView() {
  const { chat } = useShell();
  const { groupId, sessionId, setGroupId, startNewChat, selectChat } = chat;

  const { currentSessionId } = useSessionTab();

  const model = useLanguageModel("chat");
  const userId = useCurrentUserId();

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
        chat.mode !== "RightPanelOpen" && "bg-stone-50",
      ])}
    >
      <ChatHeader
        currentChatGroupId={groupId}
        onNewChat={startNewChat}
        onSelectChat={selectChat}
        handleClose={() => chat.sendEvent({ type: "CLOSE" })}
      />
      {userId && (
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
