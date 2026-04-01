import { create } from "zustand";

import { id } from "~/shared/utils";

interface ChatContextState {
  groupId: string | undefined;
  sessionId: string;
}

interface ChatContextActions {
  setGroupId: (groupId: string | undefined) => void;
  startNewChat: () => void;
  selectChat: (groupId: string) => void;
}

export const useChatContext = create<ChatContextState & ChatContextActions>(
  (set) => ({
    groupId: undefined,
    sessionId: id(),
    setGroupId: (groupId) => set({ groupId }),
    startNewChat: () => set({ groupId: undefined, sessionId: id() }),
    selectChat: (groupId) => set({ groupId, sessionId: groupId }),
  }),
);
