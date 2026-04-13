import { useEffect, useRef } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";

import { useShell } from "~/contexts/shell";
import { useSearch } from "~/search/contexts/ui";
import { LeftSidebar } from "~/sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainSidebar() {
  const { leftsidebar } = useShell();
  const { query } = useSearch();
  const currentTab = useTabs((state) => state.currentTab);
  const isOnboarding = currentTab?.type === "onboarding";
  const previousQueryRef = useRef(query);

  const hasCustomSidebar =
    currentTab?.type === "calendar" ||
    currentTab?.type === "settings" ||
    currentTab?.type === "contacts" ||
    currentTab?.type === "templates" ||
    currentTab?.type === "prompts";
  const savedExpandedRef = useRef<boolean | null>(null);
  const wasCustomSidebarRef = useRef(false);

  useEffect(() => {
    if (hasCustomSidebar && !wasCustomSidebarRef.current) {
      savedExpandedRef.current = leftsidebar.expanded;
      if (!leftsidebar.expanded) {
        leftsidebar.setExpanded(true);
        windowsCommands
          .windowExpandWidth(280, null, false, true)
          .catch(console.error);
      }
      leftsidebar.setLocked(true);
    } else if (!hasCustomSidebar && wasCustomSidebarRef.current) {
      leftsidebar.setLocked(false);
      if (savedExpandedRef.current !== null) {
        leftsidebar.setExpanded(savedExpandedRef.current);
      }
      savedExpandedRef.current = null;
      windowsCommands.windowRestoreWidth().catch(console.error);
    }
    wasCustomSidebarRef.current = hasCustomSidebar;
  }, [hasCustomSidebar, leftsidebar]);

  useEffect(() => {
    const isStartingSearch =
      query.trim() !== "" && previousQueryRef.current.trim() === "";

    if (isStartingSearch && !leftsidebar.expanded && !isOnboarding) {
      leftsidebar.setExpanded(true);
      windowsCommands
        .windowExpandWidth(280, null, false, true)
        .catch(console.error);
    }

    previousQueryRef.current = query;
  }, [query, leftsidebar, isOnboarding]);

  if (!leftsidebar.expanded || isOnboarding) {
    return null;
  }

  return <LeftSidebar />;
}
