import { ClassicMainBody } from "./body";

import { useShell } from "~/contexts/shell";
import { useConfigValue } from "~/shared/config";
import { MainShellBodyFrame, MainShellScaffold } from "~/shared/main";
import { hasCustomSidebarTab } from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainShellFrame() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");

  const isOnboarding = currentTab?.type === "onboarding";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const showSidebarTimeline =
    sidebarTimelineEnabled &&
    leftsidebar.expanded &&
    !leftsidebar.showDevtool &&
    !hasCustomSidebar &&
    !isOnboarding;
  const showTopTimeline =
    leftsidebar.expanded &&
    !showSidebarTimeline &&
    !leftsidebar.showDevtool &&
    !hasCustomSidebar &&
    !isOnboarding;

  return (
    <MainShellScaffold edgeToEdge={showTopTimeline}>
      <MainShellBodyFrame>
        <ClassicMainBody />
      </MainShellBodyFrame>
    </MainShellScaffold>
  );
}
