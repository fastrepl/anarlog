import { useHotkeys } from "react-hotkeys-hook";
import { useShallow } from "zustand/shallow";

import { useBillingAccess } from "~/auth/billing";
import { useShell } from "~/contexts/shell";
import { useNewNote, useNewNoteAndListen } from "~/shared/useNewNote";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function useMainTabsShortcuts({ onModT }: { onModT: () => void }) {
  const {
    tabs,
    currentTab,
    close,
    select,
    selectNext,
    selectPrev,
    restoreLastClosedTab,
    openNew,
    unpin,
    setPendingCloseConfirmationTab,
  } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
      close: state.close,
      select: state.select,
      selectNext: state.selectNext,
      selectPrev: state.selectPrev,
      restoreLastClosedTab: state.restoreLastClosedTab,
      openNew: state.openNew,
      unpin: state.unpin,
      setPendingCloseConfirmationTab: state.setPendingCloseConfirmationTab,
    })),
  );
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isListening = liveStatus === "active" || liveStatus === "finalizing";
  const { isPro } = useBillingAccess();
  const { chat } = useShell();

  const newNote = useNewNote();
  const newNoteCurrent = useNewNote({ behavior: "current" });

  useHotkeys(
    "mod+n",
    () => {
      if (isPersistentChatInputFocused(chat.mode)) {
        chat.startNewChat();
        return;
      }

      if (currentTab?.type === "empty") {
        newNoteCurrent();
      } else {
        newNote();
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [chat, currentTab, newNote, newNoteCurrent],
  );

  useHotkeys(
    "mod+t",
    () => onModT(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [onModT],
  );

  useHotkeys(
    "mod+w",
    async () => {
      if (currentTab) {
        const isCurrentTabListening =
          isListening &&
          currentTab.type === "sessions" &&
          currentTab.id === liveSessionId;
        if (isCurrentTabListening) {
          setPendingCloseConfirmationTab(currentTab);
        } else if (currentTab.pinned) {
          unpin(currentTab);
        } else {
          if (currentTab.type === "chat_support") {
            chat.sendEvent({ type: "CLOSE" });
          }
          close(currentTab);
        }
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [
      currentTab,
      close,
      unpin,
      isListening,
      liveSessionId,
      setPendingCloseConfirmationTab,
      chat,
    ],
  );

  useHotkeys(
    "mod+1, mod+2, mod+3, mod+4, mod+5, mod+6, mod+7, mod+8, mod+9",
    (event) => {
      const key = event.key;
      const targetIndex =
        key === "9" ? tabs.length - 1 : Number.parseInt(key, 10) - 1;
      const target = tabs[targetIndex];
      if (target) {
        select(target);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [tabs, select],
  );

  useHotkeys(
    "mod+alt+left",
    () => selectPrev(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [selectPrev],
  );

  useHotkeys(
    "mod+alt+right",
    () => selectNext(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [selectNext],
  );

  useHotkeys(
    "mod+shift+t",
    () => restoreLastClosedTab(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [restoreLastClosedTab],
  );

  useHotkeys(
    "mod+shift+c",
    () => openNew({ type: "calendar" }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+o",
    () =>
      openNew({
        type: "contacts",
        state: { selected: null },
      }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+comma",
    () => openNew({ type: "settings", state: { tab: "transcription" } }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+l",
    () => {
      if (!isPro) {
        return;
      }

      openNew({ type: "folders", id: null });
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [isPro, openNew],
  );

  const newNoteAndListen = useNewNoteAndListen();

  useHotkeys(
    "mod+shift+n",
    () => newNoteAndListen(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [newNoteAndListen],
  );

  return {};
}

function isPersistentChatInputFocused(
  mode: ReturnType<typeof useShell>["chat"]["mode"],
) {
  if (mode !== "FloatingOpen" && mode !== "RightPanelOpen") {
    return false;
  }

  if (typeof document === "undefined") {
    return false;
  }

  const activeElement = document.activeElement;
  if (!(activeElement instanceof HTMLElement)) {
    return false;
  }

  return activeElement.closest("[data-chat-message-input]") !== null;
}
