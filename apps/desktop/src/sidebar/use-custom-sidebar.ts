import { useEffect, useRef } from "react";

import type { Tab } from "~/store/zustand/tabs";

const CUSTOM_SIDEBAR_TYPES: Tab["type"][] = [
  "calendar",
  "settings",
  "contacts",
  "templates",
  "prompts",
];

export function hasCustomSidebarTab(tab: Tab | null): boolean {
  return tab !== null && CUSTOM_SIDEBAR_TYPES.includes(tab.type);
}

export function useCustomSidebarEffect(
  active: boolean,
  leftsidebar: {
    expanded: boolean;
    setExpanded: (v: boolean) => void;
    setLocked: (v: boolean) => void;
  },
  { restoreExpandedOnExit = true }: { restoreExpandedOnExit?: boolean } = {},
) {
  const savedExpandedRef = useRef<boolean | null>(null);
  const wasActiveRef = useRef(false);

  useEffect(() => {
    if (active && !wasActiveRef.current) {
      savedExpandedRef.current = leftsidebar.expanded;
      if (!leftsidebar.expanded) {
        leftsidebar.setExpanded(true);
      }
      leftsidebar.setLocked(true);
    } else if (!active && wasActiveRef.current) {
      leftsidebar.setLocked(false);
      if (restoreExpandedOnExit && savedExpandedRef.current !== null) {
        leftsidebar.setExpanded(savedExpandedRef.current);
      } else if (!restoreExpandedOnExit) {
        leftsidebar.setExpanded(false);
      }
      savedExpandedRef.current = null;
    }
    wasActiveRef.current = active;
  }, [active, leftsidebar, restoreExpandedOnExit]);
}
