import type { StateCreator, StoreMutatorIdentifier } from "zustand";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import {
  getDefaultState,
  type Tab,
  type TabInput,
  uniqueIdfromTab,
} from "./schema";

import { commands } from "~/types/tauri.gen";

export type PinnedTab = TabInput & { pinned: true };

const serializePinnedTabs = (tabs: Tab[]): string => {
  const pinnedTabs = tabs
    .filter((t) => t.pinned)
    .map((tab): PinnedTab => {
      const { active, slotId, pinned, ...rest } = tab as Tab & {
        active: boolean;
        slotId: string;
        pinned: boolean;
      };
      return { ...rest, pinned: true } as PinnedTab;
    });
  return JSON.stringify(pinnedTabs);
};

const deserializePinnedTabs = (data: string): PinnedTab[] => {
  try {
    return JSON.parse(data) as PinnedTab[];
  } catch {
    return [];
  }
};

export const savePinnedTabs = async (tabs: Tab[]): Promise<void> => {
  const serialized = serializePinnedTabs(tabs);
  await commands.setPinnedTabs(serialized);
};

export const loadPinnedTabs = async (): Promise<PinnedTab[]> => {
  const result = await commands.getPinnedTabs();
  if (result.status === "ok" && result.data) {
    return deserializePinnedTabs(result.data);
  }
  return [];
};

type PinnedPersistenceMiddleware = <
  T extends {
    tabs: Tab[];
  },
  Mps extends [StoreMutatorIdentifier, unknown][] = [],
  Mcs extends [StoreMutatorIdentifier, unknown][] = [],
>(
  f: StateCreator<T, Mps, Mcs>,
) => StateCreator<T, Mps, Mcs>;

type PinnedPersistenceMiddlewareImpl = <
  T extends {
    tabs: Tab[];
  },
>(
  f: StateCreator<T, [], []>,
) => StateCreator<T, [], []>;

const getPinnedTabIds = (tabs: Tab[]): string[] => {
  return tabs
    .filter((t) => t.pinned)
    .map(uniqueIdfromTab)
    .sort();
};

const pinnedPersistenceMiddlewareImpl: PinnedPersistenceMiddlewareImpl =
  (config) => (set, get, api) => {
    return config(
      (args) => {
        const prevState = get();
        const prevPinnedIds = getPinnedTabIds(prevState.tabs);

        set(args);

        const nextState = get();
        const nextPinnedIds = getPinnedTabIds(nextState.tabs);

        const pinnedChanged =
          prevPinnedIds.length !== nextPinnedIds.length ||
          prevPinnedIds.some((id, i) => id !== nextPinnedIds[i]);

        if (pinnedChanged && getCurrentWebviewWindowLabel() === "main") {
          savePinnedTabs(nextState.tabs).catch((e) => {
            console.error("Failed to save pinned tabs:", e);
          });
        }
      },
      get,
      api,
    );
  };

export const pinnedPersistenceMiddleware =
  pinnedPersistenceMiddlewareImpl as PinnedPersistenceMiddleware;

export const restorePinnedTabsToStore = async (
  openNew: (tab: TabInput) => void,
  pin: (tab: Tab) => void,
  getTabs: () => Tab[],
): Promise<void> => {
  const pinnedTabs = await loadPinnedTabs();

  for (const pinnedTab of pinnedTabs) {
    const { pinned, ...tabInput } = pinnedTab;
    openNew(tabInput);

    const tabs = getTabs();
    const newTab = tabs.find((t) => {
      const tabWithDefaults = getDefaultState(tabInput);
      return uniqueIdfromTab(t) === uniqueIdfromTab(tabWithDefaults as Tab);
    });

    if (newTab && !newTab.pinned) {
      pin(newTab);
    }
  }
};
