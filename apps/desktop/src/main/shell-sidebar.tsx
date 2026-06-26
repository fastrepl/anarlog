import { useShell } from "~/contexts/shell";
import { LeftSidebar } from "~/sidebar";
import {
  hasCustomSidebarTab,
  useCustomSidebarEffect,
} from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainSidebar({
  forceMount = false,
  showIgnoredTimelineEvents,
  onShowIgnoredTimelineEventsChange,
}: {
  forceMount?: boolean;
  showIgnoredTimelineEvents?: boolean;
  onShowIgnoredTimelineEventsChange?: (showIgnored: boolean) => void;
} = {}) {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const isOnboarding = currentTab?.type === "onboarding";

  const hasCustomSidebar = hasCustomSidebarTab(currentTab);

  useCustomSidebarEffect(hasCustomSidebar, leftsidebar);

  if ((!leftsidebar.expanded && !forceMount) || isOnboarding) {
    return null;
  }

  return (
    <LeftSidebar
      showIgnoredTimelineEvents={showIgnoredTimelineEvents}
      onShowIgnoredTimelineEventsChange={onShowIgnoredTimelineEventsChange}
    />
  );
}
