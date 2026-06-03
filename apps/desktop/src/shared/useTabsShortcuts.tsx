import { useCallback, useRef } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useShallow } from "zustand/shallow";

import { useShell } from "~/contexts/shell";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useNewNote, useNewNoteAndListen } from "~/shared/useNewNote";
import { uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function useMainTabsShortcuts({ onModT }: { onModT: () => void }) {
  const runEscapeShortcut = useMainEscapeShortcutAction();
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
  const { chat } = useShell();

  const newNote = useNewNote();
  const newNoteCurrent = useNewNote({ behavior: "current" });

  const escapeShortcutRef = useRef(runEscapeShortcut);
  escapeShortcutRef.current = runEscapeShortcut;
  const chatRef = useRef(chat);
  chatRef.current = chat;

  useMountEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      const fromProseMirrorEditor = isFromProseMirrorEditor(event.target);
      const hadEditorEscapeConsumer =
        fromProseMirrorEditor &&
        document.querySelector("[data-editor-escape-consumer]") !== null;
      const hadOpenChat = chatRef.current.mode !== "FloatingClosed";

      window.setTimeout(() => {
        if (
          shouldSkipEscapeShortcut(event, {
            fromProseMirrorEditor,
            hadEditorEscapeConsumer,
          })
        ) {
          return;
        }

        if (hadOpenChat) {
          chatRef.current.sendEvent({ type: "CLOSE" });
          return;
        }

        escapeShortcutRef.current();
      });
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  });

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
    "mod+shift+comma",
    () => openNew({ type: "settings", state: { tab: "transcription" } }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
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

  return { runEscapeShortcut };
}

export function useMainEscapeShortcutAction() {
  const { chat } = useShell();

  return useCallback(() => {
    if (chat.mode !== "FloatingClosed") {
      chat.sendEvent({ type: "CLOSE" });
      return;
    }

    const { tabs, currentTab, openCurrent, select, goBack, canGoBack } =
      useTabs.getState();

    if (currentTab?.type === "onboarding" || currentTab?.type === "empty") {
      return;
    }

    if (currentTab?.type === "sessions") {
      return;
    }

    const returnToSlotId = currentTab?.returnToSlotId;
    const returnToTab = returnToSlotId
      ? tabs.find(
          (tab) =>
            tab.slotId === returnToSlotId &&
            tab.slotId !== currentTab?.slotId &&
            (!currentTab?.returnToTabId ||
              uniqueIdfromTab(tab) === currentTab.returnToTabId),
        )
      : null;
    if (returnToTab) {
      select(returnToTab);
      return;
    }

    if (returnToSlotId === currentTab?.slotId && canGoBack) {
      goBack();
      return;
    }

    const existingHomeTab = tabs.find((tab) => tab.type === "empty");
    if (existingHomeTab) {
      select(existingHomeTab);
      return;
    }

    openCurrent({ type: "empty" });
  }, [chat.mode, chat.sendEvent]);
}

function shouldSkipEscapeShortcut(
  event: KeyboardEvent,
  {
    fromProseMirrorEditor,
    hadEditorEscapeConsumer,
  }: { fromProseMirrorEditor: boolean; hadEditorEscapeConsumer: boolean },
) {
  if (!event.defaultPrevented) {
    return false;
  }

  if (!fromProseMirrorEditor) {
    return true;
  }

  return hadEditorEscapeConsumer;
}

function isFromProseMirrorEditor(target: EventTarget | null) {
  return target instanceof Element && target.closest(".ProseMirror") !== null;
}

function isPersistentChatInputFocused(
  mode: ReturnType<typeof useShell>["chat"]["mode"],
) {
  if (mode === "FloatingClosed") {
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
