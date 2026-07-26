import type { StoreApi } from "zustand";

import type { Tab, TabInput } from "./schema";

const MAX_CLOSED_TABS = 10;

export type RestoreState = {
  closedTabs: Tab[];
};

export type RestoreActions = {
  restoreLastClosedTab: () => void;
};

const tabToTabInput = (tab: Tab): TabInput => {
  const { active, slotId, pinned, returnToSlotId, returnToTabId, ...rest } =
    tab as Tab & {
      active: boolean;
      slotId: string;
      pinned: boolean;
      returnToSlotId?: string;
      returnToTabId?: string;
    };
  return rest as TabInput;
};

export const createRestoreSlice = <
  T extends RestoreState & { tabs: Tab[]; openNew: (tab: TabInput) => void },
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): RestoreState & RestoreActions => ({
  closedTabs: [],
  restoreLastClosedTab: () => {
    const { closedTabs, openNew } = get();
    if (closedTabs.length === 0) return;

    const lastClosed = closedTabs[closedTabs.length - 1];
    const remainingClosedTabs = closedTabs.slice(0, -1);

    openNew(tabToTabInput(lastClosed));
    set({ closedTabs: remainingClosedTabs } as Partial<T>);
  },
});

type RestoreMiddleware = <
  T extends {
    tabs: Tab[];
    closedTabs: Tab[];
  },
>(
  f: (
    set: StoreApi<T>["setState"],
    get: StoreApi<T>["getState"],
    api: StoreApi<T>,
  ) => T,
) => (
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
  api: StoreApi<T>,
) => T;

const restoreMiddlewareImpl: RestoreMiddleware =
  (config) => (set, get, api) => {
    return config(
      (args) => {
        const prevState = get();
        const prevTabs = prevState.tabs;

        set(args);

        const nextState = get();
        const nextTabs = nextState.tabs;

        const closedTabs = prevTabs.filter(
          (prevTab) =>
            prevTab.type !== "shared_note_preview" &&
            !nextTabs.some((nextTab) => nextTab.slotId === prevTab.slotId),
        );

        if (closedTabs.length > 0) {
          const updatedClosedTabs = [
            ...nextState.closedTabs,
            ...closedTabs,
          ].slice(-MAX_CLOSED_TABS);
          set({ closedTabs: updatedClosedTabs } as Partial<typeof nextState>);
        }
      },
      get,
      api,
    );
  };

export const restoreMiddleware = restoreMiddlewareImpl;
