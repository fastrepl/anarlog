import { create } from "zustand";

import { id } from "~/shared/utils";

interface ChatContextState {
  groupId: string | undefined;
  sessionId: string;
}

interface ChatContextActions {
  setGroupId: (groupId: string | undefined) => void;
  rollbackFailedGroup: (failedGroupId: string) => void;
  startNewChat: () => void;
  selectChat: (groupId: string) => void;
}

export const useChatContext = create<ChatContextState & ChatContextActions>(
  (set) => ({
    groupId: undefined,
    sessionId: id(),
    setGroupId: (groupId) => set({ groupId }),
    // Compares against the live groupId, not a value captured when the send
    // started — the failure lands after onGroupCreated already updated it.
    rollbackFailedGroup: (failedGroupId) =>
      set((state) =>
        state.groupId === failedGroupId ? { groupId: undefined } : state,
      ),
    startNewChat: () => set({ groupId: undefined, sessionId: id() }),
    selectChat: (groupId) => set({ groupId, sessionId: groupId }),
  }),
);
