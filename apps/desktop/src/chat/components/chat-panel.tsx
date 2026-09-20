import { useLingui } from "@lingui/react/macro";
import { type ReactNode, useCallback } from "react";

import { ChatCircle, Sparkle } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

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
import { LiveAssistPanel } from "~/session/components/live-assist";
import { useSessionHasTranscript } from "~/session/queries";
import { useOwnerUserId } from "~/shared/owner-user";
import { folderIdForNewNote, useSidebarNotes } from "~/sidebar/note-filter";
import { isBatchTranscriptionPending } from "~/store/zustand/listener/general-shared";
import { useLiveAssistPanelTab } from "~/store/zustand/live-assist/panel-tab";
import { useListener } from "~/stt/contexts";

export function ChatSessionHost({
  children,
}: {
  children: (sessionProps: ChatSessionRenderProps | null) => ReactNode;
}) {
  const { chat } = useShell();
  const { groupId, sessionId } = chat;
  const { currentSessionId } = useSessionTab();
  const folderFilter = useSidebarNotes((state) => state.folderFilter);
  const contextSessionId =
    chat.scope === "automations" ? undefined : currentSessionId;
  const folderId =
    chat.scope === "automations" ? undefined : folderIdForNewNote(folderFilter);
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
  const { t } = useLingui();
  const { chat } = useShell();
  const { groupId, setGroupId, rollbackFailedGroup } = chat;
  const { panelClassName, toolbarSurface } = useChatAppearance();
  const isFloating = layout === "floating";
  const model = useLanguageModel("chat");
  const { currentSessionId } = useSessionTab();
  const liveAssistPanelTab = useLiveAssistPanelTab((state) => state.activeTab);
  const setLiveAssistPanelTab = useLiveAssistPanelTab(
    (state) => state.setActiveTab,
  );
  // The Chat/Live Assist switcher only makes sense in the right panel, next
  // to an active note session; the floating chat and the automations scope
  // (which hides the toolbar entirely) are left untouched.
  const showLiveAssistSwitcher =
    !isFloating && chat.scope !== "automations" && Boolean(currentSessionId);
  const showLiveAssistPanel =
    showLiveAssistSwitcher && liveAssistPanelTab === "live_assist";

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
      {showLiveAssistSwitcher && (
        <div className="flex shrink-0 px-3 pb-2">
          <div className="bg-muted flex rounded-md p-0.5">
            <button
              type="button"
              aria-pressed={liveAssistPanelTab === "chat"}
              onClick={() => setLiveAssistPanelTab("chat")}
              className={cn([
                "flex h-6 items-center gap-1 rounded-sm px-2 text-xs font-medium transition-colors",
                liveAssistPanelTab === "chat"
                  ? "bg-card text-foreground shadow-xs"
                  : "text-muted-foreground hover:text-foreground",
              ])}
            >
              <ChatCircle aria-hidden className="size-3.5" />
              {t`Chat`}
            </button>
            <button
              type="button"
              aria-pressed={liveAssistPanelTab === "live_assist"}
              onClick={() => setLiveAssistPanelTab("live_assist")}
              className={cn([
                "flex h-6 items-center gap-1 rounded-sm px-2 text-xs font-medium transition-colors",
                liveAssistPanelTab === "live_assist"
                  ? "bg-card text-foreground shadow-xs"
                  : "text-muted-foreground hover:text-foreground",
              ])}
            >
              <Sparkle aria-hidden className="size-3.5" />
              {t`Live Assist`}
            </button>
          </div>
        </div>
      )}
      {showLiveAssistPanel && currentSessionId ? (
        <LiveAssistPanel sessionId={currentSessionId} />
      ) : (
        sessionProps && (
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
        )
      )}
    </div>
  );
}
