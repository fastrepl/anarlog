import { useHotkeys } from "react-hotkeys-hook";

import {
  selectCanGoBack,
  selectCanGoNext,
  selectCurrentTab,
  useTabsStore,
} from "~/tabs/tabs.store";
import { type Tab, uniqueIdFromTab } from "~/tabs/tabs.types";

const HOTKEY_OPTIONS = {
  preventDefault: true,
  enableOnFormTags: true,
  enableOnContentEditable: true,
} as const;

export type TabsShortcutsOptions = {
  // Called on `mod+n` (and `mod+shift+n` until STT lands). Typically wired to
  // an ad-hoc session-creation mutation in the shell container.
  onNewNote?: () => void;
};

export function useTabsShortcuts(options: TabsShortcutsOptions = {}) {
  const { onNewNote } = options;

  const tabs = useTabsStore((state) => state.tabs);
  const currentTab = useTabsStore(selectCurrentTab);
  const close = useTabsStore((state) => state.close);
  const select = useTabsStore((state) => state.select);
  const openNew = useTabsStore((state) => state.openNew);
  const clearSelection = useTabsStore((state) => state.clearSelection);
  const restoreLastClosedTab = useTabsStore(
    (state) => state.restoreLastClosedTab,
  );
  const goBack = useTabsStore((state) => state.goBack);
  const goNext = useTabsStore((state) => state.goNext);
  const canGoBack = useTabsStore(selectCanGoBack);
  const canGoNext = useTabsStore(selectCanGoNext);

  useHotkeys(
    "mod+w",
    () => {
      if (currentTab) {
        close(uniqueIdFromTab(currentTab));
        return;
      }

      window.close();
    },
    HOTKEY_OPTIONS,
    [close, currentTab],
  );

  useHotkeys(
    "mod+n",
    () => {
      onNewNote?.();
    },
    HOTKEY_OPTIONS,
    [onNewNote],
  );

  useHotkeys(
    "mod+t",
    () => {
      clearSelection();
    },
    HOTKEY_OPTIONS,
    [clearSelection],
  );

  useHotkeys(
    "mod+shift+t",
    () => {
      restoreLastClosedTab();
    },
    HOTKEY_OPTIONS,
    [restoreLastClosedTab],
  );

  // Desktop2 has no listener stack yet, so `mod+shift+n` maps to the same
  // new-note action as `mod+n`; once STT lands we can flip this to
  // "new note + start listening" without touching the keybinding.
  useHotkeys(
    "mod+shift+n",
    () => {
      onNewNote?.();
    },
    HOTKEY_OPTIONS,
    [onNewNote],
  );

  useHotkeys(
    "mod+shift+c",
    () => openAndSelect(openNew, { type: "calendar" }),
    HOTKEY_OPTIONS,
    [openNew],
  );

  useHotkeys(
    "mod+shift+o",
    () => openAndSelect(openNew, { type: "contacts" }),
    HOTKEY_OPTIONS,
    [openNew],
  );

  useHotkeys(
    "mod+shift+comma",
    () => openAndSelect(openNew, { type: "settings" }),
    HOTKEY_OPTIONS,
    [openNew],
  );

  useHotkeys(
    "mod+shift+l",
    () => openAndSelect(openNew, { type: "folders" }),
    HOTKEY_OPTIONS,
    [openNew],
  );

  useHotkeys(
    "mod+1, mod+2, mod+3, mod+4, mod+5, mod+6, mod+7, mod+8, mod+9",
    (event) => {
      const key = event.key;
      const targetIndex =
        key === "9" ? tabs.length - 1 : Number.parseInt(key, 10) - 1;
      const tab = tabs[targetIndex];
      if (tab) {
        select(uniqueIdFromTab(tab));
      }
    },
    HOTKEY_OPTIONS,
    [select, tabs],
  );

  useHotkeys(
    "mod+alt+left",
    () => {
      if (canGoBack) {
        goBack();
      }
    },
    HOTKEY_OPTIONS,
    [canGoBack, goBack],
  );

  useHotkeys(
    "mod+alt+right",
    () => {
      if (canGoNext) {
        goNext();
      }
    },
    HOTKEY_OPTIONS,
    [canGoNext, goNext],
  );
}

function openAndSelect(openNew: (tab: Tab) => void, tab: Tab) {
  openNew(tab);
}
