import { create } from "zustand";

import { isStarterId, type StarterId } from "./starters";

import { useChatContext } from "~/chat/state/chat-context";
import { useStoredSettingValue } from "~/settings/queries";
import { id } from "~/shared/utils";

export type AutomationSelection =
  | { kind: "starter"; starterId: StarterId }
  | { kind: "chat"; groupId: string }
  | { kind: "draft"; draftId: string };

type ChatSelection = { groupId: string | undefined; sessionId: string };

interface AutomationSelectionState {
  selection: AutomationSelection | null;
  draftIds: string[];
  chatBySelection: Record<string, ChatSelection>;
  selectStarter: (starterId: StarterId) => void;
  selectChatAutomation: (groupId: string) => void;
  selectDraft: (draftId: string) => void;
  startDraft: () => void;
  removeDraft: (draftId: string) => void;
  clearSelection: (selection: AutomationSelection) => void;
}

function selectionChatKey(selection: AutomationSelection): string {
  switch (selection.kind) {
    case "starter":
      return `starter:${selection.starterId}`;
    case "chat":
      return `chat:${selection.groupId}`;
    case "draft":
      return `draft:${selection.draftId}`;
  }
}

export const useAutomationSelection = create<AutomationSelectionState>(
  (set, get) => {
    // Each automation keeps its own chat thread: switching selection snapshots
    // the live automations-scope chat under the outgoing selection and
    // restores the incoming selection's thread.
    const switchTo = (
      selection: AutomationSelection,
      { freshChat = false }: { freshChat?: boolean } = {},
    ) => {
      const { selection: previous, chatBySelection } = get();
      const liveChat = useChatContext.getState().chatByScope.automations;
      const nextChatBySelection = previous
        ? { ...chatBySelection, [selectionChatKey(previous)]: liveChat }
        : chatBySelection;

      const restored = freshChat
        ? undefined
        : nextChatBySelection[selectionChatKey(selection)];
      const nextChat =
        restored ??
        (selection.kind === "chat"
          ? { groupId: selection.groupId, sessionId: selection.groupId }
          : { groupId: undefined, sessionId: id() });

      set({ selection, chatBySelection: nextChatBySelection });
      useChatContext.setState((state) => ({
        chatByScope: { ...state.chatByScope, automations: nextChat },
      }));
    };

    const clearSelection = (selection: AutomationSelection) => {
      const { selection: current, chatBySelection } = get();
      const key = selectionChatKey(selection);
      const nextChatBySelection = { ...chatBySelection };
      delete nextChatBySelection[key];

      const isCurrent = current !== null && selectionChatKey(current) === key;
      set({
        selection: isCurrent ? null : current,
        chatBySelection: nextChatBySelection,
      });

      if (isCurrent) {
        useChatContext.setState((state) => ({
          chatByScope: {
            ...state.chatByScope,
            automations: { groupId: undefined, sessionId: id() },
          },
        }));
      }
    };

    return {
      selection: null,
      draftIds: [],
      chatBySelection: {},
      selectStarter: (starterId) => switchTo({ kind: "starter", starterId }),
      selectChatAutomation: (groupId) => switchTo({ kind: "chat", groupId }),
      selectDraft: (draftId) => switchTo({ kind: "draft", draftId }),
      startDraft: () => {
        const draftId = id();
        set((state) => ({ draftIds: [draftId, ...state.draftIds] }));
        switchTo({ kind: "draft", draftId }, { freshChat: true });
      },
      removeDraft: (draftId) => {
        set((state) => ({
          draftIds: state.draftIds.filter((item) => item !== draftId),
        }));
        clearSelection({ kind: "draft", draftId });
      },
      // For removed automations: drops the stored chat thread, and if the
      // removed automation is the selected one, falls back to the overview
      // with a fresh chat.
      clearSelection,
    };
  },
);

// A draft becomes a real automation the moment its chat creates a persisted
// group: hand the selection over to the group so the sidebar row and the
// detail view follow it.
useChatContext.subscribe((chatState) => {
  const { selection, draftIds, chatBySelection } =
    useAutomationSelection.getState();
  if (selection?.kind !== "draft") {
    return;
  }

  const groupId = chatState.chatByScope.automations.groupId;
  if (!groupId) {
    return;
  }

  const nextChatBySelection = { ...chatBySelection };
  delete nextChatBySelection[selectionChatKey(selection)];
  useAutomationSelection.setState({
    selection: { kind: "chat", groupId },
    draftIds: draftIds.filter((item) => item !== selection.draftId),
    chatBySelection: nextChatBySelection,
  });
});

export function useEffectiveAutomationSelection(): AutomationSelection | null {
  const selection = useAutomationSelection((state) => state.selection);
  const storedDraft = useStoredSettingValue("automation_draft_template");

  if (selection) {
    return selection;
  }

  return isStarterId(storedDraft.value)
    ? { kind: "starter", starterId: storedDraft.value }
    : null;
}
