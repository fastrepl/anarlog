import { create } from "zustand";

import type { ChatScope } from "~/chat/types";
import { id } from "~/shared/utils";

type ChatSelection = {
  groupId: string | undefined;
  sessionId: string;
};

interface ChatContextState {
  chatByScope: Record<ChatScope, ChatSelection>;
}

interface ChatContextActions {
  setGroupId: (scope: ChatScope, groupId: string | undefined) => void;
  rollbackFailedGroup: (scope: ChatScope, failedGroupId: string) => void;
  startNewChat: (scope: ChatScope) => void;
  selectChat: (scope: ChatScope, groupId: string) => void;
}

export const useChatContext = create<ChatContextState & ChatContextActions>(
  (set) => ({
    chatByScope: {
      general: createChatSelection(),
      automations: createChatSelection(),
    },
    setGroupId: (scope, groupId) =>
      set((state) => ({
        chatByScope: {
          ...state.chatByScope,
          [scope]: { ...state.chatByScope[scope], groupId },
        },
      })),
    // Compares against the live groupId, not a value captured when the send
    // started — the failure lands after onGroupCreated already updated it.
    rollbackFailedGroup: (scope, failedGroupId) =>
      set((state) => {
        const selection = state.chatByScope[scope];
        if (selection.groupId !== failedGroupId) {
          return state;
        }

        return {
          chatByScope: {
            ...state.chatByScope,
            [scope]: { ...selection, groupId: undefined },
          },
        };
      }),
    startNewChat: (scope) =>
      set((state) => ({
        chatByScope: {
          ...state.chatByScope,
          [scope]: createChatSelection(),
        },
      })),
    selectChat: (scope, groupId) =>
      set((state) => ({
        chatByScope: {
          ...state.chatByScope,
          [scope]: { groupId, sessionId: groupId },
        },
      })),
  }),
);

function createChatSelection(): ChatSelection {
  return { groupId: undefined, sessionId: id() };
}
