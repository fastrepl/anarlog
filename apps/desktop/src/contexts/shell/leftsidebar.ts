import {
  type Dispatch,
  type SetStateAction,
  useCallback,
  useState,
} from "react";

import { useConfigValue } from "~/shared/config";

export function useLeftSidebar() {
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const [storedExpanded, setStoredExpanded] = useState(true);
  const [locked, setLocked] = useState(false);
  const expanded = sidebarTimelineEnabled || storedExpanded;
  const effectiveLocked = sidebarTimelineEnabled || locked;

  const setExpanded: Dispatch<SetStateAction<boolean>> = useCallback(
    (next) => {
      if (sidebarTimelineEnabled) return;

      setStoredExpanded(next);
    },
    [sidebarTimelineEnabled],
  );

  const toggleExpanded = useCallback(() => {
    if (effectiveLocked) return;
    setStoredExpanded((prev) => !prev);
  }, [effectiveLocked]);

  return {
    expanded,
    setExpanded,
    locked: effectiveLocked,
    setLocked,
    toggleExpanded,
  };
}
