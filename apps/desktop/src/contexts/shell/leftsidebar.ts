import { useCallback, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { useConfigValue } from "~/shared/config";

export function useLeftSidebar() {
  const [expanded, setExpanded] = useState(true);
  const [locked, setLocked] = useState(false);
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");

  const toggleExpanded = useCallback(() => {
    if (locked) return;
    setExpanded((prev) => !prev);
  }, [locked]);

  useHotkeys(
    "mod+\\",
    toggleExpanded,
    {
      enabled: sidebarTimelineEnabled,
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [sidebarTimelineEnabled, toggleExpanded],
  );

  return {
    expanded,
    setExpanded,
    locked,
    setLocked,
    toggleExpanded,
  };
}
