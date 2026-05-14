import { platform } from "@tauri-apps/plugin-os";
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

export function ChatView() {
  const { chat } = useShell();
  const { groupId, sessionId, setGroupId } = chat;
  const currentPlatform = platform();
  const chatPanelShortcutLabel = currentPlatform === "macos" ? "⌘ J" : "Ctrl J";

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
      <div className="flex h-10 shrink-0 items-center border-b border-neutral-100 pr-0 pl-0">
        <ChatToolbarControls
          currentChatGroupId={groupId}
          onNewChat={chat.startNewChat}
          onSelectChat={chat.selectChat}
          onCloseChat={() => chat.sendEvent({ type: "CLOSE" })}
          shortcutLabel={chatPanelShortcutLabel}
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
