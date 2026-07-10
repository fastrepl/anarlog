import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

import { isSessionEmpty, softDeleteSession } from "~/session/queries";
import { listenerStore } from "~/store/zustand/listener/instance";
import {
  restorePinnedTabsToStore,
  restoreRecentlyOpenedToStore,
  type Tab,
  useTabs,
} from "~/store/zustand/tabs";

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
  invalidateSessionResource: (sessionId: string) => void;
  getSessionMode?: (sessionId: string) => string | null | undefined;
  isSessionEmptyFn?: typeof isSessionEmpty;
  deleteSessionFn?: typeof softDeleteSession;
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
  invalidateSessionResource,
  getSessionMode = (sessionId) =>
    listenerStore.getState().getSessionMode(sessionId),
  isSessionEmptyFn = isSessionEmpty,
  deleteSessionFn = softDeleteSession,
}: SessionTabCloseHandlerOptions) {
  return (tab: Tab) => {
    if (tab.type !== "sessions") {
      return;
    }

    const sessionId = tab.id;
    const sessionMode = getSessionMode(sessionId);
    if (
      sessionMode === "active" ||
      sessionMode === "finalizing" ||
      sessionMode === "running_batch"
    ) {
      return;
    }

    void (async () => {
      if (!(await isSessionEmptyFn(sessionId))) return;

      const deleted = await deleteSessionFn(sessionId);
      if (deleted) invalidateSessionResource(sessionId);
    })().catch((error) => {
      console.error("session close cleanup", error);
    });
  };
}

export function useDesktopTabLifecycle({
  onEmpty,
  onZeroTabs,
}: {
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
    registerOnClose(
      createSessionTabCloseHandler({
        invalidateSessionResource: (sessionId) => {
          useTabs.getState().invalidateResource("sessions", sessionId);
        },
      }),
    );
  }, [registerOnClose]);
}
