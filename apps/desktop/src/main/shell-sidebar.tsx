import { useEffect, useRef } from "react";

import { useShell } from "~/contexts/shell";
import { useSearch } from "~/search/contexts/ui";
import { LeftSidebar } from "~/sidebar";
import {
  hasCustomSidebarTab,
  useCustomSidebarEffect,
} from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainSidebar() {
  const { leftsidebar } = useShell();
  const { query } = useSearch();
  const currentTab = useTabs((state) => state.currentTab);
  const isOnboarding = currentTab?.type === "onboarding";
  const previousQueryRef = useRef(query);

  const hasCustomSidebar = hasCustomSidebarTab(currentTab);

  useCustomSidebarEffect(hasCustomSidebar, leftsidebar);

  useEffect(() => {
    const isStartingSearch =
      query.trim() !== "" && previousQueryRef.current.trim() === "";

    if (isStartingSearch && !leftsidebar.expanded && !isOnboarding) {
      leftsidebar.setExpanded(true);
    }

    previousQueryRef.current = query;
  }, [query, leftsidebar, isOnboarding]);

  if (!leftsidebar.expanded || isOnboarding) {
    return null;
  }

  return <LeftSidebar />;
}
