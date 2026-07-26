import type { StateCreator, StoreApi, StoreMutatorIdentifier } from "zustand";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { commands } from "~/types/tauri.gen";

const MAX_RECENT_SESSIONS = 10;

export type RecentlyOpenedState = {
  recentlyOpenedSessionIds: string[];
};

export type RecentlyOpenedActions = {
  addRecentlyOpened: (sessionId: string) => void;
};

export const createRecentlyOpenedSlice = <T extends RecentlyOpenedState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): RecentlyOpenedState & RecentlyOpenedActions => ({
  recentlyOpenedSessionIds: [],
  addRecentlyOpened: (sessionId: string) => {
    const { recentlyOpenedSessionIds } = get();
    const filtered = recentlyOpenedSessionIds.filter((id) => id !== sessionId);
    const updated = [sessionId, ...filtered].slice(0, MAX_RECENT_SESSIONS);
    set({ recentlyOpenedSessionIds: updated } as Partial<T>);
  },
});

export const saveRecentlyOpenedSessions = async (
  sessionIds: string[],
): Promise<void> => {
  const serialized = JSON.stringify(sessionIds);
  await commands.setRecentlyOpenedSessions(serialized);
};

export const loadRecentlyOpenedSessions = async (): Promise<string[]> => {
  const result = await commands.getRecentlyOpenedSessions();
  if (result.status === "ok" && result.data) {
    try {
      const parsed = JSON.parse(result.data);
      if (
        Array.isArray(parsed) &&
        parsed.every((id) => typeof id === "string")
      ) {
        return parsed;
      }
      return [];
    } catch {
      return [];
    }
  }
  return [];
};

type RecentlyOpenedMiddleware = <
  T extends {
    recentlyOpenedSessionIds: string[];
  },
  Mps extends [StoreMutatorIdentifier, unknown][] = [],
  Mcs extends [StoreMutatorIdentifier, unknown][] = [],
>(
  f: StateCreator<T, Mps, Mcs>,
) => StateCreator<T, Mps, Mcs>;

type RecentlyOpenedMiddlewareImpl = <
  T extends {
    recentlyOpenedSessionIds: string[];
  },
>(
  f: StateCreator<T, [], []>,
) => StateCreator<T, [], []>;

const recentlyOpenedMiddlewareImpl: RecentlyOpenedMiddlewareImpl =
  (config) => (set, get, api) => {
    return config(
      (args) => {
        const prevState = get();
        const prevIds = prevState.recentlyOpenedSessionIds;

        set(args);

        const nextState = get();
        const nextIds = nextState.recentlyOpenedSessionIds;

        const idsChanged =
          prevIds.length !== nextIds.length ||
          prevIds.some((id, i) => id !== nextIds[i]);

        if (idsChanged && getCurrentWebviewWindowLabel() === "main") {
          saveRecentlyOpenedSessions(nextIds).catch((e) => {
            console.error("Failed to save recently opened sessions:", e);
          });
        }
      },
      get,
      api,
    );
  };

export const recentlyOpenedMiddleware =
  recentlyOpenedMiddlewareImpl as RecentlyOpenedMiddleware;

export const restoreRecentlyOpenedToStore = async (
  set: (ids: string[]) => void,
): Promise<void> => {
  const sessionIds = await loadRecentlyOpenedSessions();
  set(sessionIds);
};
