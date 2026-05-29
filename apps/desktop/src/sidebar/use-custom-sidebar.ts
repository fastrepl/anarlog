import { useEffect, useRef } from "react";

import type { Tab } from "~/store/zustand/tabs";

const CUSTOM_SIDEBAR_TYPES: Tab["type"][] = [
  "calendar",
  "settings",
  "contacts",
  "templates",
];

const LEFT_SURFACE_CUSTOM_SIDEBAR_TYPES: Tab["type"][] = [
  "calendar",
  "settings",
  "contacts",
  "templates",
];

export function hasCustomSidebarTab(tab: Tab | null): boolean {
  return tab !== null && CUSTOM_SIDEBAR_TYPES.includes(tab.type);
}

export function hasLeftSurfaceCustomSidebarTab(tab: Tab | null): boolean {
  return tab !== null && LEFT_SURFACE_CUSTOM_SIDEBAR_TYPES.includes(tab.type);
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
  const leftsidebarRef = useRef(leftsidebar);
  const restoreExpandedOnExitRef = useRef(restoreExpandedOnExit);

  leftsidebarRef.current = leftsidebar;
  restoreExpandedOnExitRef.current = restoreExpandedOnExit;

  const releaseCustomSidebar = () => {
    if (!wasActiveRef.current) {
      return;
    }

    const currentLeftSidebar = leftsidebarRef.current;

    currentLeftSidebar.setLocked(false);
    if (restoreExpandedOnExitRef.current && savedExpandedRef.current !== null) {
      currentLeftSidebar.setExpanded(savedExpandedRef.current);
    } else if (!restoreExpandedOnExitRef.current) {
      currentLeftSidebar.setExpanded(false);
    }

    savedExpandedRef.current = null;
    wasActiveRef.current = false;
  };

  useEffect(() => {
    if (active && !wasActiveRef.current) {
      const currentLeftSidebar = leftsidebarRef.current;

      savedExpandedRef.current = currentLeftSidebar.expanded;
      if (!currentLeftSidebar.expanded) {
        currentLeftSidebar.setExpanded(true);
      }
      currentLeftSidebar.setLocked(true);
      wasActiveRef.current = true;
    } else if (!active && wasActiveRef.current) {
      releaseCustomSidebar();
    }

    return releaseCustomSidebar;
  }, [active]);
}
