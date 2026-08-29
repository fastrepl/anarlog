import { type ReactNode } from "react";

import { useShell } from "~/contexts/shell";
import { LeftSidebar } from "~/sidebar";
import type { SidebarNoteFilter } from "~/sidebar/note-filter";
import {
  hasCustomSidebarTab,
  useCustomSidebarEffect,
} from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainSidebar({
  folderFilter = null,
  noteFilter = "mine",
  timelineHeader,
  showIgnoredTimelineEvents,
  onShowIgnoredTimelineEventsChange,
}: {
  folderFilter?: string | null;
  noteFilter?: SidebarNoteFilter;
  timelineHeader?: ReactNode;
  showIgnoredTimelineEvents?: boolean;
  onShowIgnoredTimelineEventsChange?: (showIgnored: boolean) => void;
} = {}) {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const isOnboarding = currentTab?.type === "onboarding";

  const hasCustomSidebar = hasCustomSidebarTab(currentTab);

  useCustomSidebarEffect(hasCustomSidebar, leftsidebar);

  if (!leftsidebar.expanded || isOnboarding) {
    return null;
  }

  return (
    <LeftSidebar
      folderFilter={folderFilter}
      noteFilter={noteFilter}
      timelineHeader={timelineHeader}
      showIgnoredTimelineEvents={showIgnoredTimelineEvents}
      onShowIgnoredTimelineEventsChange={onShowIgnoredTimelineEventsChange}
    />
  );
}
