import { useCallback, useState } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatHeader } from "./header";
import { ChatSession } from "./session-provider";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { useShell } from "~/contexts/shell";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";

export function ChatView() {
  const { chat } = useShell();
  const { groupId, setGroupId } = chat;

  const { currentSessionId } = useSessionTab();

  // sessionId drives the ChatSession key and useChat id.
  // It is managed explicitly — not derived from groupId — so that we can distinguish:
  //   handleNewChat:    new random ID → fresh useChat instance
  //   handleSelectChat: set to groupId → forces ChatSession remount to load history
  //   onGroupCreated:   groupId changes but sessionId stays stable → keeps useChat alive for the in-flight stream
  const [sessionId, setSessionId] = useState<string>(() => groupId ?? id());

  const model = useLanguageModel("chat");
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const handleGroupCreated = useCallback(
    (newGroupId: string) => {
      // Don't update sessionId — keep current one so useChat stays alive for the in-flight stream
      setGroupId(newGroupId);
    },
    [setGroupId],
  );

  const { handleSendMessage } = useChatActions({
    groupId,
    onGroupCreated: handleGroupCreated,
  });

  const handleNewChat = useCallback(() => {
    setGroupId(undefined);
    setSessionId(id());
  }, [setGroupId]);

  const handleSelectChat = useCallback(
    (selectedGroupId: string) => {
      setGroupId(selectedGroupId);
      setSessionId(selectedGroupId);
    },
    [setGroupId],
  );

  return (
    <div
      className={cn([
        "flex h-full min-h-0 flex-col overflow-hidden",
        chat.mode !== "RightPanelOpen" && "bg-stone-50",
      ])}
    >
      <ChatHeader
        currentChatGroupId={groupId}
        onNewChat={handleNewChat}
        onSelectChat={handleSelectChat}
        handleClose={() => chat.sendEvent({ type: "CLOSE" })}
      />
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
