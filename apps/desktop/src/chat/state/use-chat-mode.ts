import { useHotkeys } from "react-hotkeys-hook";

import { commands as windowsCommands } from "@hypr/plugin-windows";

import { useChatContext } from "./chat-context";

import { useTabs } from "~/store/zustand/tabs";

export type { ChatEvent, ChatMode } from "~/store/zustand/tabs";

export function useChatMode() {
  const mode = useTabs((state) => state.chatMode);
  const transitionChatMode = useTabs((state) => state.transitionChatMode);

  const groupId = useChatContext((state) => state.groupId);
  const sessionId = useChatContext((state) => state.sessionId);
  const setGroupId = useChatContext((state) => state.setGroupId);
  const startNewChat = useChatContext((state) => state.startNewChat);
  const selectChat = useChatContext((state) => state.selectChat);

  useHotkeys(
    "mod+j",
    () => {
      windowsCommands.windowShow({ type: "composer" }).catch(console.error);
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [],
  );

  return {
    mode,
    sendEvent: transitionChatMode,
    groupId,
    sessionId,
    setGroupId,
    startNewChat,
    selectChat,
  };
}
