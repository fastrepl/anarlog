import { useCallback, useRef } from "react";

import { useTabs } from "~/store/zustand/tabs";

export function useSessionTab() {
  const { currentTab } = useTabs();

  const sessionTabId =
    currentTab?.type === "sessions" ? currentTab.id : undefined;
  const enhancedNoteId =
    currentTab?.type === "sessions" &&
    currentTab.state.view?.type === "enhanced"
      ? currentTab.state.view.id
      : undefined;

  const stickySessionIdRef = useRef(sessionTabId);
  if (sessionTabId) {
    stickySessionIdRef.current = sessionTabId;
  }

  const activeSessionIdRef = useRef(sessionTabId);
  activeSessionIdRef.current = sessionTabId;

  const enhancedNoteIdRef = useRef(enhancedNoteId);
  enhancedNoteIdRef.current = enhancedNoteId;

  const getSessionId = useCallback(() => activeSessionIdRef.current, []);
  const getEnhancedNoteId = useCallback(() => enhancedNoteIdRef.current, []);

  return {
    currentSessionId: stickySessionIdRef.current,
    getSessionId,
    getEnhancedNoteId,
  };
}
