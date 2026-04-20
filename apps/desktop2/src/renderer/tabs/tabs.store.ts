import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { type Tab, uniqueIdFromTab } from "~/tabs/tabs.types";

type TabsState = {
  tabs: Tab[];
  currentId: string | null;
  history: Array<string | null>;
  historyIndex: number;
  // Stack of recently-closed tabs so `mod+shift+t` can restore the last one.
  recentlyClosed: Tab[];
  openNew: (tab: Tab) => void;
  select: (id: string) => void;
  close: (id: string) => void;
  reorder: (tabs: Tab[]) => void;
  clearSelection: () => void;
  goBack: () => void;
  goNext: () => void;
  restoreLastClosedTab: () => void;
};

const RECENTLY_CLOSED_LIMIT = 10;

function nextHistory(
  history: Array<string | null>,
  historyIndex: number,
  nextId: string | null,
) {
  if (history[historyIndex] === nextId) {
    return { history, historyIndex };
  }

  const nextHistory = history.slice(0, historyIndex + 1);
  nextHistory.push(nextId);

  return {
    history: nextHistory,
    historyIndex: nextHistory.length - 1,
  };
}

export const useTabsStore = create<TabsState>()(
  persist(
    (set, get) => ({
      tabs: [],
      currentId: null,
      history: [null],
      historyIndex: 0,
      recentlyClosed: [],
      openNew: (tab) => {
        set((state) => {
          const tabId = uniqueIdFromTab(tab);
          const existingTab = state.tabs.find(
            (current) => uniqueIdFromTab(current) === tabId,
          );
          const tabs = existingTab ? state.tabs : [...state.tabs, tab];
          const next = nextHistory(state.history, state.historyIndex, tabId);

          return {
            tabs,
            currentId: tabId,
            history: next.history,
            historyIndex: next.historyIndex,
          };
        });
      },
      select: (id) => {
        set((state) => {
          const exists = state.tabs.some((tab) => uniqueIdFromTab(tab) === id);
          if (!exists) {
            return state;
          }

          const next = nextHistory(state.history, state.historyIndex, id);
          return {
            currentId: id,
            history: next.history,
            historyIndex: next.historyIndex,
          };
        });
      },
      close: (id) => {
        set((state) => {
          const index = state.tabs.findIndex(
            (tab) => uniqueIdFromTab(tab) === id,
          );
          if (index === -1) {
            return state;
          }

          const closedTab = state.tabs[index]!;
          const tabs = state.tabs.filter((tab) => uniqueIdFromTab(tab) !== id);
          const currentId =
            state.currentId === id
              ? tabs[index - 1]
                ? uniqueIdFromTab(tabs[index - 1]!)
                : tabs[0]
                  ? uniqueIdFromTab(tabs[0]!)
                  : null
              : state.currentId;
          const next = nextHistory(
            state.history,
            state.historyIndex,
            currentId,
          );
          const recentlyClosed = [closedTab, ...state.recentlyClosed].slice(
            0,
            RECENTLY_CLOSED_LIMIT,
          );

          return {
            tabs,
            currentId,
            history: next.history,
            historyIndex: next.historyIndex,
            recentlyClosed,
          };
        });
      },
      reorder: (tabs) => {
        set((state) => {
          const nextIds = new Set(tabs.map(uniqueIdFromTab));
          const missing = state.tabs.filter(
            (tab) => !nextIds.has(uniqueIdFromTab(tab)),
          );

          return {
            tabs: [...tabs, ...missing],
          };
        });
      },
      clearSelection: () => {
        set((state) => {
          const next = nextHistory(state.history, state.historyIndex, null);
          return {
            currentId: null,
            history: next.history,
            historyIndex: next.historyIndex,
          };
        });
      },
      goBack: () => {
        const state = get();
        if (state.historyIndex <= 0) {
          return;
        }

        set({
          currentId: state.history[state.historyIndex - 1] ?? null,
          historyIndex: state.historyIndex - 1,
        });
      },
      goNext: () => {
        const state = get();
        if (state.historyIndex >= state.history.length - 1) {
          return;
        }

        set({
          currentId: state.history[state.historyIndex + 1] ?? null,
          historyIndex: state.historyIndex + 1,
        });
      },
      restoreLastClosedTab: () => {
        set((state) => {
          const [head, ...rest] = state.recentlyClosed;
          if (!head) {
            return state;
          }

          const tabId = uniqueIdFromTab(head);
          const alreadyOpen = state.tabs.some(
            (tab) => uniqueIdFromTab(tab) === tabId,
          );
          const tabs = alreadyOpen ? state.tabs : [...state.tabs, head];
          const next = nextHistory(state.history, state.historyIndex, tabId);

          return {
            tabs,
            currentId: tabId,
            history: next.history,
            historyIndex: next.historyIndex,
            recentlyClosed: rest,
          };
        });
      },
    }),
    {
      name: "hypr-desktop2.tabs.v1",
      storage: createJSONStorage(() => window.localStorage),
    },
  ),
);

export function selectCurrentTab(state: TabsState): Tab | null {
  if (!state.currentId) {
    return null;
  }

  return (
    state.tabs.find((tab) => uniqueIdFromTab(tab) === state.currentId) ?? null
  );
}

export function selectCanGoBack(state: TabsState): boolean {
  return state.historyIndex > 0;
}

export function selectCanGoNext(state: TabsState): boolean {
  return state.historyIndex < state.history.length - 1;
}
