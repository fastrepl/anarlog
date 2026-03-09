import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatHeader } from "./header";
import { ChatSession } from "./session-provider";
import { useEditSummaryTools } from "./use-edit-summary-tools";
import { useSessionTab } from "./use-session-tab";

import { useLanguageModel } from "~/ai/hooks";
import {
  useChatActions,
  useStableSessionId,
} from "~/chat/store/use-chat-actions";
import { useShell } from "~/contexts/shell";

export function ChatView() {
  const { chat } = useShell();
  const { groupId, setGroupId } = chat;

  const { currentSessionId, getSessionId, getEnhancedNoteId } = useSessionTab();
  const { extraTools } = useEditSummaryTools(getSessionId, getEnhancedNoteId);

  const stableSessionId = useStableSessionId(groupId);
  const model = useLanguageModel("chat");

  const { handleSendMessage } = useChatActions({
    groupId,
    onGroupCreated: setGroupId,
  });

  const handleNewChat = useCallback(() => {
    setGroupId(undefined);
  }, [setGroupId]);

  const handleSelectChat = useCallback(
    (selectedGroupId: string) => {
      setGroupId(selectedGroupId);
    },
    [setGroupId],
  );

  return (
    <div
      className={cn([
        "flex h-full min-h-0 flex-col overflow-hidden",
        chat.mode === "RightPanelOpen" &&
          "overflow-hidden rounded-xl border border-neutral-200",
      ])}
    >
      <ChatHeader
        currentChatGroupId={groupId}
        onNewChat={handleNewChat}
        onSelectChat={handleSelectChat}
        handleClose={() => chat.sendEvent({ type: "CLOSE" })}
      />
      <div className="bg-sky-100 px-3 py-1.5 text-[11px] text-neutral-900">
        NOTE: Chat is mostly READ ONLY. More editing updates coming soon.
      </div>
      <ChatSession
        key={stableSessionId}
        sessionId={stableSessionId}
        chatGroupId={groupId}
        currentSessionId={currentSessionId}
        extraTools={extraTools}
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
    </div>
  );
}
