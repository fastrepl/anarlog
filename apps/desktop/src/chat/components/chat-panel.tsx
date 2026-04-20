import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatSession } from "./session-provider";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { useShell } from "~/contexts/shell";
import * as main from "~/store/tinybase/store/main";

export function ChatView() {
  const { chat } = useShell();
  const { groupId, sessionId, setGroupId } = chat;

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
        chat.mode !== "RightPanelOpen" && "bg-stone-50",
      ])}
    >
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
