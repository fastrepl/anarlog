import { useEffect, useRef } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";

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
) {
  const savedExpandedRef = useRef<boolean | null>(null);
  const wasActiveRef = useRef(false);

  useEffect(() => {
    if (active && !wasActiveRef.current) {
      savedExpandedRef.current = leftsidebar.expanded;
      if (!leftsidebar.expanded) {
        leftsidebar.setExpanded(true);
        windowsCommands
          .windowExpandWidth(280, null, false, true)
          .catch(console.error);
      }
      leftsidebar.setLocked(true);
    } else if (!active && wasActiveRef.current) {
      leftsidebar.setLocked(false);
      if (savedExpandedRef.current !== null) {
        leftsidebar.setExpanded(savedExpandedRef.current);
      }
      savedExpandedRef.current = null;
      windowsCommands.windowRestoreWidth().catch(console.error);
    }
    wasActiveRef.current = active;
  }, [active, leftsidebar]);
}
