import type { StoreApi } from "zustand";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import type { ChatModeState } from "./chat-mode";
import type { LifecycleState } from "./lifecycle";
import type { NavigationState, TabHistory } from "./navigation";
import { pushHistory, updateHistoryCurrent } from "./navigation";
import type {
  RecentlyOpenedActions,
  RecentlyOpenedState,
} from "./recently-opened";
import { getDefaultState, isSameTab, type Tab, type TabInput } from "./schema";

import { id } from "~/shared/utils";
import { listenerStore } from "~/store/zustand/listener/instance";

export type BasicState = {
  tabs: Tab[];
  currentTab: Tab | null;
};

export type BasicActions = {
  openCurrent: (tab: TabInput) => void;
  openNew: (tab: TabInput, options?: { position?: "start" | "end" }) => void;
  select: (tab: Tab) => void;
  selectNext: () => void;
  selectPrev: () => void;
  close: (tab: Tab) => void;
  reorder: (tabs: Tab[]) => void;
  closeOthers: (tab: Tab) => void;
  closeAll: () => void;
  pin: (tab: Tab) => void;
  unpin: (tab: Tab) => void;
};

export const createBasicSlice = <
  T extends BasicState &
    NavigationState &
    LifecycleState &
    RecentlyOpenedState &
    RecentlyOpenedActions &
    ChatModeState,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): BasicState & BasicActions => ({
  tabs: [],
  currentTab: null,
  openCurrent: (tab) => {
    const { tabs, history, addRecentlyOpened } = get();
    const currentActiveTab = tabs.find((t) => t.active);

    const isCurrentTabListening =
      currentActiveTab?.type === "sessions" &&
      currentActiveTab.id === listenerStore.getState().live.sessionId &&
      (listenerStore.getState().live.status === "active" ||
        listenerStore.getState().live.status === "finalizing");

    if (currentActiveTab?.pinned || isCurrentTabListening) {
      set(openTab(tabs, tab, history, true));
    } else {
      set(openTab(tabs, tab, history, false));
    }

    if (tab.type === "sessions") {
      addRecentlyOpened(tab.id);
    }

    void analyticsCommands.event({
      event: "tab_opened",
      view: tab.type,
    });
  },
  openNew: (tab, options) => {
    const { tabs, history, addRecentlyOpened } = get();
    set(openTab(tabs, tab, history, true, options?.position));

    if (tab.type === "sessions") {
      addRecentlyOpened(tab.id);
    }

    void analyticsCommands.event({
      event: "tab_opened",
      view: tab.type,
    });
  },
  select: (tab) => {
    const { tabs, addRecentlyOpened } = get();
    const nextTabs = setActiveFlags(tabs, tab);
    const currentTab = nextTabs.find((t) => t.active) || null;
    set({ tabs: nextTabs, currentTab } as Partial<T>);

    if (tab.type === "sessions") {
      addRecentlyOpened(tab.id);
    }
  },
  selectNext: () => {
    const { tabs, currentTab } = get();
    if (tabs.length === 0 || !currentTab) return;

    const currentIndex = tabs.findIndex((t) => isSameTab(t, currentTab));
    const nextIndex = (currentIndex + 1) % tabs.length;
    const nextTab = tabs[nextIndex];

    const nextTabs = setActiveFlags(tabs, nextTab);
    set({
      tabs: nextTabs,
      currentTab: { ...nextTab, active: true },
    } as Partial<T>);
  },
  selectPrev: () => {
    const { tabs, currentTab } = get();
    if (tabs.length === 0 || !currentTab) return;

    const currentIndex = tabs.findIndex((t) => isSameTab(t, currentTab));
    const prevIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    const prevTab = tabs[prevIndex];

    const nextTabs = setActiveFlags(tabs, prevTab);
    set({
      tabs: nextTabs,
      currentTab: { ...prevTab, active: true },
    } as Partial<T>);
  },
  close: (tab) => {
    const { tabs, history, canClose } = get();
    const tabToClose = tabs.find((t) => isSameTab(t, tab));

    if (!tabToClose) {
      return;
    }

    if (canClose && !canClose(tabToClose)) {
      return;
    }

    const shouldResetChatMode =
      tabToClose.type === "chat_support" && get().chatMode === "FullTab";
    const remainingTabs = tabs.filter((t) => !isSameTab(t, tab));

    if (remainingTabs.length === 0) {
      set({
        tabs: [],
        currentTab: null,
        history: new Map(),
        canGoBack: false,
        canGoNext: false,
        ...(shouldResetChatMode ? { chatMode: "FloatingClosed" as const } : {}),
      } as unknown as Partial<T>);
      return;
    }

    const closedTabIndex = tabs.findIndex((t) => isSameTab(t, tab));
    const nextActiveIndex = findNextActiveIndex(remainingTabs, closedTabIndex);
    const nextTabs = setActiveFlags(
      remainingTabs,
      remainingTabs[nextActiveIndex],
    );
    const nextCurrentTab = nextTabs[nextActiveIndex];

    const nextHistory = new Map(history);
    nextHistory.delete(tabToClose.slotId);

    set({
      tabs: nextTabs,
      currentTab: nextCurrentTab,
      history: nextHistory,
      ...(shouldResetChatMode ? { chatMode: "FloatingClosed" as const } : {}),
    } as Partial<T>);
  },
  reorder: (tabs) => {
    const currentTab = tabs.find((t) => t.active) || null;
    set({ tabs, currentTab } as Partial<T>);
  },
  closeOthers: (tab) => {
    const { tabs, history } = get();
    const tabToKeep = tabs.find((t) => isSameTab(t, tab));

    if (!tabToKeep) {
      return;
    }

    const isRemovingChatTab =
      tabToKeep.type !== "chat_support" &&
      tabs.some((t) => t.type === "chat_support");
    const shouldResetChatMode =
      isRemovingChatTab && get().chatMode === "FullTab";

    const nextHistory = new Map(history);
    const tabWithActiveFlag = { ...tabToKeep, active: true };
    const nextTabs = [tabWithActiveFlag];

    Array.from(history.keys()).forEach((slotId) => {
      if (slotId !== tabToKeep.slotId) {
        nextHistory.delete(slotId);
      }
    });

    set({
      tabs: nextTabs,
      currentTab: tabWithActiveFlag,
      history: nextHistory,
      ...(shouldResetChatMode ? { chatMode: "FloatingClosed" as const } : {}),
    } as Partial<T>);
  },
  closeAll: () => {
    const shouldResetChatMode = get().chatMode === "FullTab";
    set({
      tabs: [],
      currentTab: null,
      history: new Map(),
      canGoBack: false,
      canGoNext: false,
      ...(shouldResetChatMode ? { chatMode: "FloatingClosed" as const } : {}),
    } as unknown as Partial<T>);
  },
  pin: (tab) => {
    const { tabs } = get();
    const tabIndex = tabs.findIndex((t) => isSameTab(t, tab));
    if (tabIndex === -1) return;

    const pinnedTab = { ...tabs[tabIndex], pinned: true };
    const pinnedCount = tabs.filter((t) => t.pinned).length;

    const nextTabs = [...tabs.slice(0, tabIndex), ...tabs.slice(tabIndex + 1)];
    nextTabs.splice(pinnedCount, 0, pinnedTab);

    const currentTab = nextTabs.find((t) => t.active) || null;
    set({ tabs: nextTabs, currentTab } as Partial<T>);
  },
  unpin: (tab) => {
    const { tabs } = get();
    const tabIndex = tabs.findIndex((t) => isSameTab(t, tab));
    if (tabIndex === -1) return;

    const unpinnedTab = { ...tabs[tabIndex], pinned: false };
    const pinnedCount = tabs.filter((t) => t.pinned).length;

    const nextTabs = [...tabs.slice(0, tabIndex), ...tabs.slice(tabIndex + 1)];
    nextTabs.splice(pinnedCount - 1, 0, unpinnedTab);

    const currentTab = nextTabs.find((t) => t.active) || null;
    set({ tabs: nextTabs, currentTab } as Partial<T>);
  },
});

const setActiveFlags = (tabs: Tab[], activeTab: Tab): Tab[] => {
  return tabs.map((t) => ({ ...t, active: isSameTab(t, activeTab) }));
};

const deactivateAll = (tabs: Tab[]): Tab[] => {
  return tabs.map((t) => ({ ...t, active: false }));
};

const findNextActiveIndex = (tabs: Tab[], closedIndex: number): number => {
  return closedIndex < tabs.length ? closedIndex : tabs.length - 1;
};

const updateWithHistory = <T extends BasicState & NavigationState>(
  tabs: Tab[],
  currentTab: Tab,
  history: Map<string, TabHistory>,
): Partial<T> => {
  const nextHistory = pushHistory(history, currentTab);
  return { tabs, currentTab, history: nextHistory } as Partial<T>;
};

const openTab = <T extends BasicState & NavigationState>(
  tabs: Tab[],
  newTab: TabInput,
  history: Map<string, TabHistory>,
  forceNewTab: boolean,
  position?: "start" | "end",
): Partial<T> => {
  const tabWithDefaults: Tab = {
    ...getDefaultState(newTab),
    active: false,
    slotId: id(),
  };

  let nextTabs: Tab[];
  let activeTab: Tab;

  const existingTab = tabs.find((t) => isSameTab(t, tabWithDefaults));
  const isNewTab = !existingTab;

  if (!isNewTab) {
    const nextExistingTab = reuseExistingTab(existingTab!, tabWithDefaults);
    nextTabs = tabs.map((tab) =>
      isSameTab(tab, existingTab!)
        ? { ...nextExistingTab, active: true }
        : { ...tab, active: false },
    );
    const currentTab = { ...nextExistingTab, active: true };
    return {
      tabs: nextTabs,
      currentTab,
      history: updateHistoryCurrent(history, currentTab),
    } as Partial<T>;
  }

  if (!forceNewTab) {
    const existingActiveIdx = tabs.findIndex((t) => t.active);
    const currentActiveTab = tabs[existingActiveIdx];

    if (existingActiveIdx !== -1 && currentActiveTab) {
      activeTab = {
        ...tabWithDefaults,
        active: true,
        slotId: currentActiveTab.slotId,
      };

      nextTabs = tabs.map((t, idx) => {
        if (idx === existingActiveIdx) {
          return activeTab;
        }
        return { ...t, active: false };
      });
    } else {
      activeTab = { ...tabWithDefaults, active: true, slotId: id() };
      const deactivated = deactivateAll(tabs);
      nextTabs = [...deactivated, activeTab];
    }

    return updateWithHistory(nextTabs, activeTab, history);
  } else {
    activeTab = { ...tabWithDefaults, active: true, slotId: id() };
    const deactivated = deactivateAll(tabs);

    if (position === "start") {
      const pinnedCount = deactivated.filter((t) => t.pinned).length;
      nextTabs = [
        ...deactivated.slice(0, pinnedCount),
        activeTab,
        ...deactivated.slice(pinnedCount),
      ];
    } else {
      nextTabs = [...deactivated, activeTab];
    }

    return updateWithHistory(nextTabs, activeTab, history);
  }
};

const reuseExistingTab = (existingTab: Tab, requestedTab: Tab): Tab => {
  if (existingTab.type === "settings" && requestedTab.type === "settings") {
    return {
      ...existingTab,
      state: requestedTab.state,
    };
  }

  return existingTab;
};
