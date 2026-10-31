import { type ReactNode, useCallback, useState } from "react";

import { cn } from "@anlg/utils";

import { ChatBody } from "./body";
import { ChatContent } from "./content";
import { ChatModelSelector } from "./model-selector";
import { ChatSession, type ChatSessionRenderProps } from "./session-provider";
import { ChatToolbarControls } from "./toolbar-controls";
import { useSessionTab } from "./use-session-tab";
import { useHasEditableSummary } from "./use-summary-quick-actions";

import { useLanguageModel } from "~/ai/hooks";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useChatContext } from "~/chat/state/chat-context";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { chatFloatingPanelClassNames } from "~/chat/surface";
import type { ChatScope } from "~/chat/types";
import { useShell } from "~/contexts/shell";
import { useSessionHasTranscript } from "~/session/queries";
import { useOwnerUserId } from "~/shared/owner-user";
import { folderIdForNewNote, useSidebarNotes } from "~/sidebar/note-filter";
import { isBatchTranscriptionPending } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";

export function ChatSessionHost({
  children,
}: {
  children: (sessionProps: ChatSessionRenderProps | null) => ReactNode;
}) {
  const { chat } = useShell();
  const [scopes, setScopes] = useState<ChatScope[]>([chat.scope]);
  if (!scopes.includes(chat.scope)) {
    setScopes([...scopes, chat.scope]);
  }

  return scopes.map((scope) => (
    <ScopedChatSession key={scope} scope={scope}>
      {(sessionProps) => (scope === chat.scope ? children(sessionProps) : null)}
    </ScopedChatSession>
  ));
}

function ScopedChatSession({
  scope,
  children,
}: {
  scope: ChatScope;
  children: (sessionProps: ChatSessionRenderProps | null) => ReactNode;
}) {
  const { groupId, sessionId } = useChatContext(
    (state) => state.chatByScope[scope],
  );
  const { currentSessionId } = useSessionTab();
  const noteFilter = useSidebarNotes((state) => state.noteFilter);
  const folderFilter = useSidebarNotes((state) => state.folderFilter);
  const contextSessionId =
    scope === "automations" ? undefined : currentSessionId;
  const folderId =
    scope === "automations"
      ? undefined
      : folderIdForNewNote(noteFilter, folderFilter);
  const ownerUserId = useOwnerUserId();
  const hasAvailableTranscript = useSessionHasTranscript(
    contextSessionId ?? "",
  );
  const batchTranscriptionPending = useListener((state) => {
    if (!contextSessionId) {
      return false;
    }
    return isBatchTranscriptionPending(
      state.getSessionMode(contextSessionId),
      state.live,
      state.live.batchTranscriptionPendingBySession[contextSessionId],
    );
  });

  if (!ownerUserId) {
    return <>{children(null)}</>;
  }

  return (
    <ChatSession
      sessionId={sessionId}
      chatGroupId={groupId}
      currentSessionId={contextSessionId}
      folderId={folderId}
      hasAvailableTranscript={hasAvailableTranscript}
      isBatchTranscriptionPending={batchTranscriptionPending}
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
  const { groupId, setGroupId, rollbackFailedGroup } = chat;
  const { panelClassName, toolbarSurface } = useChatAppearance();
  const isFloating = layout === "floating";
  const model = useLanguageModel("chat");
  const { currentSessionId, currentNoteView } = useSessionTab();
  const hasSummary = useHasEditableSummary(
    chat.scope === "automations" ? undefined : currentSessionId,
    currentNoteView,
  );
  const isLiveMeeting = useListener(
    (state) =>
      chat.scope !== "automations" &&
      !!currentSessionId &&
      state.getSessionMode(currentSessionId) === "active",
  );

  const handleGroupCreated = useCallback(
    (newGroupId: string) => {
      setGroupId(newGroupId);
    },
    [setGroupId],
  );

  const handleGroupCreateFailed = useCallback(
    (failedGroupId: string) => {
      rollbackFailedGroup(failedGroupId);
    },
    [rollbackFailedGroup],
  );

  const { handleSendMessage } = useChatActions({
    chatScope: chat.scope,
    groupId,
    onGroupCreated: handleGroupCreated,
    onGroupCreateFailed: handleGroupCreateFailed,
  });

  return (
    <div
      className={cn([
        "flex min-h-0 flex-col overflow-hidden",
        isFloating ? "max-h-full" : "h-full",
        isFloating ? chatFloatingPanelClassNames() : panelClassName,
      ])}
    >
      {chat.scope === "automations" ? null : (
        <div
          data-tauri-drag-region={!isFloating || undefined}
          className={cn([
            "flex shrink-0 pr-0 pl-0",
            isFloating ? "h-11 items-center" : "h-9 items-start pt-[9px]",
          ])}
        >
          <ChatToolbarControls
            chatScope={chat.scope}
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
      )}
      {sessionProps && (
        <ChatContent
          {...sessionProps}
          layout={layout}
          onDraftContentChange={onDraftContentChange}
          model={model}
          modelSelector={
            <ChatModelSelector
              disabled={
                sessionProps.status === "submitted" ||
                sessionProps.status === "streaming"
              }
            />
          }
          handleSendMessage={handleSendMessage}
        >
          <ChatBody
            messages={sessionProps.messages}
            status={sessionProps.status}
            error={sessionProps.error}
            onReload={sessionProps.regenerate}
            isModelConfigured={!!model}
            hasContext={sessionProps.contextEntities.length > 0}
            isLiveMeeting={isLiveMeeting}
            hasSummary={hasSummary && sessionProps.contextEntities.length > 0}
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
