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
import {
  getDefaultState,
  isSameTab,
  type Tab,
  type TabInput,
  uniqueIdfromTab,
} from "./schema";

import { id } from "~/shared/utils";
import { listenerStore } from "~/store/zustand/listener/instance";

const RETURN_ORIGIN_TAB_TYPES: Tab["type"][] = [
  "calendar",
  "contacts",
  "settings",
  "templates",
];

export type BasicState = {
  tabs: Tab[];
  currentTab: Tab | null;
};

export type BasicActions = {
  openCurrent: (tab: TabInput) => void;
  openNew: (tab: TabInput, options?: { position?: "start" | "end" }) => void;
  select: (tab: Tab) => void;
  clearSelection: () => void;
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
    const { tabs, history, addRecentlyOpened, chatMode } = get();
    const currentActiveTab = tabs.find((t) => t.active);
    const shouldCloseChat = shouldCloseChatForNavigation(
      currentActiveTab,
      tab,
      chatMode,
    );

    const isCurrentTabListening =
      currentActiveTab?.type === "sessions" &&
      currentActiveTab.id === listenerStore.getState().live.sessionId &&
      (listenerStore.getState().live.status === "active" ||
        listenerStore.getState().live.status === "finalizing");

    if (currentActiveTab?.pinned || isCurrentTabListening) {
      set(
        withChatCollapsedForNavigation(
          openTab(tabs, tab, history, true),
          shouldCloseChat,
        ),
      );
    } else {
      set(
        withChatCollapsedForNavigation(
          openTab(tabs, tab, history, false),
          shouldCloseChat,
        ),
      );
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
    const { tabs, history, addRecentlyOpened, chatMode } = get();
    const currentActiveTab = tabs.find((t) => t.active);
    const shouldCloseChat = shouldCloseChatForNavigation(
      currentActiveTab,
      tab,
      chatMode,
    );

    set(
      withChatCollapsedForNavigation(
        openTab(tabs, tab, history, true, options?.position),
        shouldCloseChat,
      ),
    );

    if (tab.type === "sessions") {
      addRecentlyOpened(tab.id);
    }

    void analyticsCommands.event({
      event: "tab_opened",
      view: tab.type,
    });
  },
  select: (tab) => {
    const { tabs, addRecentlyOpened, chatMode } = get();
    const currentActiveTab = tabs.find((t) => t.active);
    const shouldCloseChat = shouldCloseChatForNavigation(
      currentActiveTab,
      tab,
      chatMode,
    );
    const nextTabs = setActiveFlags(tabs, tab);
    const currentTab = nextTabs.find((t) => t.active) || null;
    set(
      withChatCollapsedForNavigation(
        { tabs: nextTabs, currentTab } as Partial<T>,
        shouldCloseChat,
      ),
    );

    if (tab.type === "sessions") {
      addRecentlyOpened(tab.id);
    }
  },
  clearSelection: () => {
    const { tabs } = get();
    set({
      tabs: tabs.map((tab) => ({ ...tab, active: false })),
      currentTab: null,
    } as Partial<T>);
  },
  selectNext: () => {
    const { tabs, currentTab, chatMode } = get();
    if (tabs.length === 0 || !currentTab) return;

    const currentIndex = tabs.findIndex((t) => isSameTab(t, currentTab));
    const nextIndex = (currentIndex + 1) % tabs.length;
    const nextTab = tabs[nextIndex];
    const shouldCloseChat = shouldCloseChatForNavigation(
      currentTab,
      nextTab,
      chatMode,
    );

    const nextTabs = setActiveFlags(tabs, nextTab);
    set(
      withChatCollapsedForNavigation(
        {
          tabs: nextTabs,
          currentTab: { ...nextTab, active: true },
        } as Partial<T>,
        shouldCloseChat,
      ),
    );
  },
  selectPrev: () => {
    const { tabs, currentTab, chatMode } = get();
    if (tabs.length === 0 || !currentTab) return;

    const currentIndex = tabs.findIndex((t) => isSameTab(t, currentTab));
    const prevIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    const prevTab = tabs[prevIndex];
    const shouldCloseChat = shouldCloseChatForNavigation(
      currentTab,
      prevTab,
      chatMode,
    );

    const nextTabs = setActiveFlags(tabs, prevTab);
    set(
      withChatCollapsedForNavigation(
        {
          tabs: nextTabs,
          currentTab: { ...prevTab, active: true },
        } as Partial<T>,
        shouldCloseChat,
      ),
    );
  },
  close: (tab) => {
    const { tabs, history, canClose, chatMode } = get();
    const tabToClose = tabs.find((t) => isSameTab(t, tab));

    if (!tabToClose) {
      return;
    }

    if (canClose && !canClose(tabToClose)) {
      return;
    }

    const remainingTabs = clearReturnOriginsForSlot(
      tabs.filter((t) => !isSameTab(t, tab)),
      tabToClose.slotId,
    );

    if (remainingTabs.length === 0) {
      set({
        tabs: [],
        currentTab: null,
        history: new Map(),
        canGoBack: false,
        canGoNext: false,
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
    const shouldCloseChat =
      tabToClose.active &&
      shouldCloseChatForNavigation(tabToClose, nextCurrentTab, chatMode);

    const nextHistory = new Map(history);
    nextHistory.delete(tabToClose.slotId);

    set(
      withChatCollapsedForNavigation(
        {
          tabs: nextTabs,
          currentTab: nextCurrentTab,
          history: nextHistory,
        } as Partial<T>,
        shouldCloseChat,
      ),
    );
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
    } as Partial<T>);
  },
  closeAll: () => {
    set({
      tabs: [],
      currentTab: null,
      history: new Map(),
      canGoBack: false,
      canGoNext: false,
    } as unknown as Partial<T>);
  },
  pin: (tab) => {
    if (tab.type === "shared_sessions" || tab.type === "shared_note_preview") {
      return;
    }

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
  const activeOriginTab = tabs.find((tab) => tab.active);
  const returnOrigin =
    shouldTrackReturnOrigin(tabWithDefaults) &&
    activeOriginTab &&
    !isSameTab(activeOriginTab, tabWithDefaults)
      ? {
          returnToSlotId: activeOriginTab.slotId,
          returnToTabId: uniqueIdfromTab(activeOriginTab),
        }
      : undefined;
  const tabWithOrigin = returnOrigin
    ? { ...tabWithDefaults, ...returnOrigin }
    : tabWithDefaults;

  let nextTabs: Tab[];
  let activeTab: Tab;

  const existingTab = tabs.find((t) => isSameTab(t, tabWithOrigin));
  const isNewTab = !existingTab;

  if (!isNewTab) {
    const nextExistingTab = reuseExistingTab(existingTab!, tabWithOrigin, {
      preserveReturnOrigin:
        existingTab!.active && !tabWithOrigin.returnToSlotId,
    });
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
        ...tabWithOrigin,
        active: true,
        slotId: currentActiveTab.slotId,
      };

      const tabsWithFreshOrigins = clearReturnOriginsForSlot(
        tabs,
        currentActiveTab.slotId,
      );
      nextTabs = tabsWithFreshOrigins.map((t, idx) => {
        if (idx === existingActiveIdx) {
          return activeTab;
        }
        return { ...t, active: false };
      });
    } else {
      activeTab = { ...tabWithOrigin, active: true, slotId: id() };
      const deactivated = deactivateAll(tabs);
      nextTabs = [...deactivated, activeTab];
    }

    return updateWithHistory(nextTabs, activeTab, history);
  } else {
    activeTab = { ...tabWithOrigin, active: true, slotId: id() };
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

const reuseExistingTab = (
  existingTab: Tab,
  requestedTab: Tab,
  {
    preserveReturnOrigin,
  }: {
    preserveReturnOrigin: boolean;
  },
): Tab => {
  if (existingTab.type === "settings" && requestedTab.type === "settings") {
    const nextTab = applyReturnOriginForReuse(
      existingTab,
      requestedTab,
      preserveReturnOrigin,
    );

    return {
      ...nextTab,
      state: requestedTab.state,
    };
  }

  return applyReturnOriginForReuse(
    existingTab,
    requestedTab,
    preserveReturnOrigin,
  );
};

const shouldTrackReturnOrigin = (tab: Tab): boolean =>
  RETURN_ORIGIN_TAB_TYPES.includes(tab.type);

const clearReturnOriginsForSlot = (tabs: Tab[], slotId: string): Tab[] => {
  return tabs.map((tab) =>
    tab.returnToSlotId === slotId ? clearReturnOrigin(tab) : tab,
  );
};

const applyReturnOriginForReuse = <T extends Tab>(
  existingTab: T,
  requestedTab: Tab,
  preserveReturnOrigin: boolean,
): T => {
  if (requestedTab.returnToSlotId) {
    return {
      ...existingTab,
      returnToSlotId: requestedTab.returnToSlotId,
      returnToTabId: requestedTab.returnToTabId,
    };
  }

  return preserveReturnOrigin ? existingTab : clearReturnOrigin(existingTab);
};

const clearReturnOrigin = <T extends Tab>(tab: T): T => {
  if (!tab.returnToSlotId && !tab.returnToTabId) {
    return tab;
  }

  const nextTab = { ...tab };
  delete nextTab.returnToSlotId;
  delete nextTab.returnToTabId;
  return nextTab;
};

const shouldCloseChatForNavigation = (
  currentTab: Tab | null | undefined,
  targetTab: Tab | TabInput,
  chatMode: ChatModeState["chatMode"],
): boolean => {
  if (chatMode === "FloatingClosed") {
    return false;
  }

  if (targetTab.type === "settings") {
    return true;
  }

  if (
    targetTab.type === "shared_sessions" ||
    targetTab.type === "shared_note_preview"
  ) {
    return true;
  }

  if (targetTab.type !== "sessions") {
    return false;
  }

  return currentTab?.type !== "sessions" || currentTab.id !== targetTab.id;
};

const withChatCollapsedForNavigation = <T extends ChatModeState>(
  state: Partial<T>,
  shouldCloseChat: boolean,
): Partial<T> => {
  if (!shouldCloseChat) {
    return state;
  }

  return {
    ...state,
    chatMode: "FloatingClosed",
  } as Partial<T>;
};
