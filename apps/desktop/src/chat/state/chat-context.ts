import { create } from "zustand";

interface ChatContextState {
  groupId: string | undefined;
}

interface ChatContextActions {
  setGroupId: (groupId: string | undefined) => void;
}

export const useChatContext = create<ChatContextState & ChatContextActions>(
  (set) => ({
    groupId: undefined,
    setGroupId: (groupId) => set({ groupId }),
  }),
);
