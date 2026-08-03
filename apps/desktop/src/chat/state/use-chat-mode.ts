import { useCallback } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { useChatContext } from "./chat-context";

import type { ChatScope } from "~/chat/types";
import { useTabs } from "~/store/zustand/tabs";

export type { ChatEvent, ChatMode } from "~/store/zustand/tabs";

export function useChatMode() {
  const mode = useTabs((state) => state.chatMode);
  const transitionChatMode = useTabs((state) => state.transitionChatMode);
  const scope = useTabs(
    (state): ChatScope =>
      state.currentTab?.type === "automations" ? "automations" : "general",
  );

  const selection = useChatContext((state) => state.chatByScope[scope]);
  const setScopedGroupId = useChatContext((state) => state.setGroupId);
  const rollbackFailedScopedGroup = useChatContext(
    (state) => state.rollbackFailedGroup,
  );
  const startNewScopedChat = useChatContext((state) => state.startNewChat);
  const selectScopedChat = useChatContext((state) => state.selectChat);

  const setGroupId = useCallback(
    (groupId: string | undefined) => setScopedGroupId(scope, groupId),
    [scope, setScopedGroupId],
  );
  const rollbackFailedGroup = useCallback(
    (failedGroupId: string) => rollbackFailedScopedGroup(scope, failedGroupId),
    [rollbackFailedScopedGroup, scope],
  );
  const startNewChat = useCallback(
    () => startNewScopedChat(scope),
    [scope, startNewScopedChat],
  );
  const selectChat = useCallback(
    (groupId: string) => selectScopedChat(scope, groupId),
    [scope, selectScopedChat],
  );

  useHotkeys(
    "mod+j",
    () => {
      transitionChatMode({ type: "TOGGLE" });
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [transitionChatMode],
  );

  return {
    mode,
    scope,
    sendEvent: transitionChatMode,
    groupId: selection.groupId,
    sessionId: selection.sessionId,
    setGroupId,
    rollbackFailedGroup,
    startNewChat,
    selectChat,
  };
}
