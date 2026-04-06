import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

import { deleteSessionCascade } from "~/store/tinybase/store/deleteSession";
import { isSessionEmpty } from "~/store/tinybase/store/sessions";
import { listenerStore } from "~/store/zustand/listener/instance";
import {
  restorePinnedTabsToStore,
  restoreRecentlyOpenedToStore,
  type Tab,
  useTabs,
} from "~/store/zustand/tabs";

type SessionStore = Parameters<typeof deleteSessionCascade>[0];
type SessionIndexes = Parameters<typeof deleteSessionCascade>[1];

type InitializeDesktopTabsOptions = {
  getTabs: () => Tab[];
  setRecentlyOpenedSessionIds: (ids: string[]) => void;
  restorePinnedTabs: () => Promise<void>;
  restoreRecentlyOpenedSessionIds: (
    set: (ids: string[]) => void,
  ) => Promise<void>;
  onZeroTabs?: (() => void) | null;
  isTauriEnv?: boolean;
};

type SessionTabCloseHandlerOptions = {
  store: SessionStore;
  indexes: SessionIndexes;
  invalidateSessionResource: (sessionId: string) => void;
  getSessionMode?: (sessionId: string) => string | null | undefined;
  isSessionEmptyFn?: typeof isSessionEmpty;
  deleteSessionFn?: typeof deleteSessionCascade;
};

export async function initializeDesktopTabs({
  getTabs,
  setRecentlyOpenedSessionIds,
  restorePinnedTabs,
  restoreRecentlyOpenedSessionIds,
  onZeroTabs,
  isTauriEnv = isTauri(),
}: InitializeDesktopTabsOptions) {
  if (!isTauriEnv) {
    onZeroTabs?.();
    return;
  }

  await restorePinnedTabs();
  await restoreRecentlyOpenedSessionIds(setRecentlyOpenedSessionIds);

  if (getTabs().length > 0) {
    return;
  }

  onZeroTabs?.();
}

export function createSessionTabCloseHandler({
  store,
  indexes,
  invalidateSessionResource,
  getSessionMode = (sessionId) =>
    listenerStore.getState().getSessionMode(sessionId),
  isSessionEmptyFn = isSessionEmpty,
  deleteSessionFn = deleteSessionCascade,
}: SessionTabCloseHandlerOptions) {
  return (tab: Tab) => {
    if (tab.type !== "sessions") {
      return;
    }

    const sessionId = tab.id;
    if (getSessionMode(sessionId) === "running_batch") {
      return;
    }

    if (!isSessionEmptyFn(store, sessionId)) {
      return;
    }

    invalidateSessionResource(sessionId);
    void deleteSessionFn(store, indexes, sessionId);
  };
}

export function useDesktopTabLifecycle({
  store,
  indexes,
  onEmpty,
  onZeroTabs,
}: {
  store: SessionStore | null | undefined;
  indexes: SessionIndexes | null | undefined;
  onEmpty?: (() => void) | null;
  onZeroTabs?: (() => void) | null;
}) {
  const { registerOnEmpty, registerCanClose, registerOnClose, openNew, pin } =
    useTabs();
  const hasOpenedInitialTab = useRef(false);

  useEffect(() => {
    if (hasOpenedInitialTab.current) {
      return;
    }

    hasOpenedInitialTab.current = true;

    void initializeDesktopTabs({
      getTabs: () => useTabs.getState().tabs,
      setRecentlyOpenedSessionIds: (ids) => {
        useTabs.setState({ recentlyOpenedSessionIds: ids });
      },
      restorePinnedTabs: () =>
        restorePinnedTabsToStore(openNew, pin, () => useTabs.getState().tabs),
      restoreRecentlyOpenedSessionIds: restoreRecentlyOpenedToStore,
      onZeroTabs,
    });
  }, [openNew, pin, onZeroTabs]);

  useEffect(() => {
    registerOnEmpty(onEmpty ?? null);
  }, [onEmpty, registerOnEmpty]);

  useEffect(() => {
    registerCanClose(() => true);
  }, [registerCanClose]);

  useEffect(() => {
    if (!store || !indexes) {
      registerOnClose(null);
      return;
    }

    registerOnClose(
      createSessionTabCloseHandler({
        store,
        indexes,
        invalidateSessionResource: (sessionId) => {
          useTabs.getState().invalidateResource("sessions", sessionId);
        },
      }),
    );
  }, [indexes, registerOnClose, store]);
}
