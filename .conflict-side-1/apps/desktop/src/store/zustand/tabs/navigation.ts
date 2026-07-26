import type { StateCreator, StoreApi, StoreMutatorIdentifier } from "zustand";

import type { BasicState } from "./basic";
import type { Tab } from "./schema";

export type NavigationState = {
  history: HistoryMap;
  canGoBack: boolean;
  canGoNext: boolean;
};

type InvalidatableResourceType = Extract<
  Tab["type"],
  "sessions" | "shared_sessions" | "humans" | "organizations"
>;

export type NavigationActions = {
  goBack: () => void;
  goNext: () => void;
  invalidateResource: (type: InvalidatableResourceType, id: string) => void;
};

export const createNavigationSlice = <T extends NavigationState & BasicState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): NavigationState & NavigationActions => ({
  history: new Map(),
  canGoBack: false,
  canGoNext: false,
  goBack: () => {
    const { tabs, history, currentTab } = get();
    if (!currentTab) {
      return;
    }

    const slotId = currentTab.slotId;
    const tabHistory = history.get(slotId);
    if (!tabHistory || tabHistory.currentIndex === 0) {
      return;
    }

    const prevIndex = tabHistory.currentIndex - 1;
    const prevTab = tabHistory.stack[prevIndex];

    const nextTabs = tabs.map((t) => (t.active ? prevTab : t));

    const nextHistory = new Map(history);
    nextHistory.set(slotId, { ...tabHistory, currentIndex: prevIndex });

    set({
      tabs: nextTabs,
      currentTab: prevTab,
      history: nextHistory,
    } as Partial<T>);
  },
  goNext: () => {
    const { tabs, history, currentTab } = get();
    if (!currentTab) {
      return;
    }

    const slotId = currentTab.slotId;
    const tabHistory = history.get(slotId);
    if (!tabHistory || tabHistory.currentIndex >= tabHistory.stack.length - 1) {
      return;
    }

    const nextIndex = tabHistory.currentIndex + 1;
    const nextTab = tabHistory.stack[nextIndex];

    const nextTabs = tabs.map((t) => (t.active ? nextTab : t));

    const nextHistory = new Map(history);
    nextHistory.set(slotId, { ...tabHistory, currentIndex: nextIndex });

    set({
      tabs: nextTabs,
      currentTab: nextTab,
      history: nextHistory,
    } as Partial<T>);
  },
  invalidateResource: (type: InvalidatableResourceType, id: string) => {
    const { history, tabs, currentTab } = get();
    const nextHistory = new Map(history);
    let hasChanges = false;

    for (const [slotId, tabHistory] of history.entries()) {
      const cleaned = cleanHistoryStack(tabHistory, (tab) =>
        isResourceMatch(tab, type, id),
      );
      if (cleaned) {
        nextHistory.set(slotId, cleaned);
      } else {
        nextHistory.delete(slotId);
      }
      if (cleaned !== tabHistory) {
        hasChanges = true;
      }
    }

    let nextTabs = tabs.filter((tab) => !isResourceMatch(tab, type, id));
    let nextCurrentTab = currentTab;

    if (currentTab && isResourceMatch(currentTab, type, id)) {
      const slotHistory = nextHistory.get(currentTab.slotId);
      const historyFallback = slotHistory?.stack[slotHistory.currentIndex];
      const otherFallback = nextTabs.find((tab) => tab.active) ?? nextTabs[0];
      const fallback: Tab | undefined =
        type === "sessions"
          ? {
              type: "empty",
              active: true,
              slotId: currentTab.slotId,
              pinned: false,
            }
          : (historyFallback ?? otherFallback);

      nextTabs = nextTabs.map((tab) => ({ ...tab, active: false }));
      if (fallback && (type === "sessions" || historyFallback)) {
        const removedIndex = tabs.findIndex((tab) =>
          isResourceMatch(tab, type, id),
        );
        nextTabs.splice(
          Math.max(0, Math.min(removedIndex, nextTabs.length)),
          0,
          { ...fallback, active: true },
        );
      } else if (fallback) {
        const fallbackIndex = nextTabs.findIndex(
          (tab) => tab.slotId === fallback.slotId,
        );
        nextTabs[fallbackIndex] = { ...nextTabs[fallbackIndex], active: true };
      }
      nextCurrentTab = nextTabs.find((tab) => tab.active) ?? null;
    }

    if (hasChanges || nextTabs.length !== tabs.length) {
      set({
        history: nextHistory,
        tabs: nextTabs,
        currentTab: nextCurrentTab,
      } as Partial<T>);
    }
  },
});

export type SlotId = string;
export type TabHistory = { stack: Tab[]; currentIndex: number };
export type HistoryMap = Map<SlotId, TabHistory>;

export const computeHistoryFlags = (
  history: Map<string, TabHistory>,
  currentTab: Tab | null,
): {
  canGoBack: boolean;
  canGoNext: boolean;
} => {
  const tabHistory = currentTab ? history.get(currentTab.slotId) : null;

  return {
    canGoBack: tabHistory ? tabHistory.currentIndex > 0 : false,
    canGoNext: tabHistory
      ? tabHistory.currentIndex < tabHistory.stack.length - 1
      : false,
  };
};

export const pushHistory = (
  history: Map<string, TabHistory>,
  tab: Tab,
): Map<string, TabHistory> => {
  if (tab.type === "empty" || tab.type === "shared_note_preview") {
    return history;
  }

  const newHistory = new Map(history);
  const slotId = tab.slotId;
  const existing = newHistory.get(slotId);

  const stack = existing
    ? [...existing.stack.slice(0, existing.currentIndex + 1), tab]
    : [tab];

  newHistory.set(slotId, { stack, currentIndex: stack.length - 1 });
  return newHistory;
};

export const updateHistoryCurrent = (
  history: Map<string, TabHistory>,
  tab: Tab,
): Map<string, TabHistory> => {
  const newHistory = new Map(history);
  const slotId = tab.slotId;
  const existing = newHistory.get(slotId);

  if (!existing) {
    return newHistory;
  }

  const stack = [...existing.stack];
  stack[existing.currentIndex] = tab;
  newHistory.set(slotId, { ...existing, stack });

  return newHistory;
};

export const isResourceMatch = (
  tab: Tab,
  type: InvalidatableResourceType,
  id: string,
): boolean => {
  if (tab.type !== type) {
    return false;
  }
  return tab.id === id;
};

export const cleanHistoryStack = (
  tabHistory: TabHistory,
  shouldRemove: (tab: Tab) => boolean,
): TabHistory | null => {
  const cleanedStack = tabHistory.stack.filter((tab) => !shouldRemove(tab));

  if (cleanedStack.length === 0) {
    return null;
  }

  let removedBeforeCurrent = 0;
  for (let i = 0; i < tabHistory.currentIndex; i++) {
    if (shouldRemove(tabHistory.stack[i])) {
      removedBeforeCurrent++;
    }
  }

  const newIndex = Math.max(
    0,
    Math.min(
      tabHistory.currentIndex - removedBeforeCurrent,
      cleanedStack.length - 1,
    ),
  );
  return { stack: cleanedStack, currentIndex: newIndex };
};

type NavigationMiddleware = <
  T extends {
    history: HistoryMap;
    currentTab: Tab | null;
    canGoBack: boolean;
    canGoNext: boolean;
  },
  Mps extends [StoreMutatorIdentifier, unknown][] = [],
  Mcs extends [StoreMutatorIdentifier, unknown][] = [],
>(
  f: StateCreator<T, Mps, Mcs>,
) => StateCreator<T, Mps, Mcs>;

const navigationMiddlewareImpl =
  <
    T extends {
      history: HistoryMap;
      currentTab: Tab | null;
      canGoBack: boolean;
      canGoNext: boolean;
    },
  >(
    config: StateCreator<T, [], []>,
  ): StateCreator<T, [], []> =>
  (set, get, api) => {
    let applyingFlags = false;

    return config(
      (args) => {
        set(args);

        if (applyingFlags) {
          return;
        }

        const state = get();
        const nextFlags = computeHistoryFlags(state.history, state.currentTab);

        if (
          state.canGoBack === nextFlags.canGoBack &&
          state.canGoNext === nextFlags.canGoNext
        ) {
          return;
        }

        applyingFlags = true;
        try {
          set(nextFlags as Partial<T>);
        } finally {
          applyingFlags = false;
        }
      },
      get,
      api,
    );
  };

export const navigationMiddleware =
  navigationMiddlewareImpl as NavigationMiddleware;
