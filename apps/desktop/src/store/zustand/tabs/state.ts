import type { StoreApi } from "zustand";

import type { BasicState } from "./basic";
import type { NavigationState } from "./navigation";
import { updateHistoryCurrent } from "./navigation";
import { isSameTab, type Tab } from "./schema";

export type StateBasicActions = {
  updateContactsTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "contacts" }>["state"],
  ) => void;
  updateSessionTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "sessions" }>["state"],
  ) => void;
  updateTemplatesTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "templates" }>["state"],
  ) => void;
  updatePromptsTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "prompts" }>["state"],
  ) => void;
  updateChatShortcutsTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "chat_shortcuts" }>["state"],
  ) => void;
  updateAiTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "ai" }>["state"],
  ) => void;
  updateSettingsTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "settings" }>["state"],
  ) => void;
  updateSearchTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "search" }>["state"],
  ) => void;
  updateChatSupportTabState: (
    tab: Tab,
    state: Extract<Tab, { type: "chat_support" }>["state"],
  ) => void;
};

export const createStateUpdaterSlice = <T extends BasicState & NavigationState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): StateBasicActions => ({
  updateSessionTabState: (tab, state) =>
    updateTabState(tab, "sessions", state, get, set),
  updateContactsTabState: (tab, state) =>
    updateTabState(tab, "contacts", state, get, set),
  updateTemplatesTabState: (tab, state) =>
    updateTabState(tab, "templates", state, get, set),
  updatePromptsTabState: (tab, state) =>
    updateTabState(tab, "prompts", state, get, set),
  updateChatShortcutsTabState: (tab, state) =>
    updateTabState(tab, "chat_shortcuts", state, get, set),
  updateAiTabState: (tab, state) => updateTabState(tab, "ai", state, get, set),
  updateSettingsTabState: (tab, state) =>
    updateTabState(tab, "settings", state, get, set),
  updateSearchTabState: (tab, state) =>
    updateTabState(tab, "search", state, get, set),
  updateChatSupportTabState: (tab, state) =>
    updateTabState(tab, "chat_support", state, get, set),
});

const updateTabState = <T extends BasicState & NavigationState>(
  tab: Tab,
  tabType: Tab["type"],
  newState: any,
  get: StoreApi<T>["getState"],
  set: StoreApi<T>["setState"],
) => {
  const { tabs, currentTab, history } = get();

  const nextTabs = tabs.map((t) =>
    isSameTab(t, tab) && t.type === tabType ? { ...t, state: newState } : t,
  );

  const nextCurrentTab =
    currentTab && isSameTab(currentTab, tab) && currentTab.type === tabType
      ? { ...currentTab, state: newState }
      : currentTab;

  const nextHistory =
    nextCurrentTab && isSameTab(nextCurrentTab, tab)
      ? updateHistoryCurrent(history, nextCurrentTab)
      : history;

  set({
    tabs: nextTabs,
    currentTab: nextCurrentTab,
    history: nextHistory,
  } as Partial<T>);
};
