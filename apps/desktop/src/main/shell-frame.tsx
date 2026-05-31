import { ClassicMainBody } from "./body";

import { useShell } from "~/contexts/shell";
import { useConfigValue } from "~/shared/config";
import { MainShellBodyFrame, MainShellScaffold } from "~/shared/main";
import { ToastArea } from "~/sidebar/toast";
import {
  hasCustomSidebarTab,
  hasLeftSurfaceCustomSidebarTab,
} from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainShellFrame() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");

  const isOnboarding = currentTab?.type === "onboarding";
  const isChangelog = currentTab?.type === "changelog";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimeline =
    sidebarTimelineEnabled &&
    leftsidebar.expanded &&
    !hasCustomSidebar &&
    !isOnboarding;
  const showTopTimeline =
    leftsidebar.expanded &&
    !showSidebarTimeline &&
    !hasCustomSidebar &&
    !isOnboarding;
  const mainSurfaceChrome =
    isChangelog && !showSidebarTimeline
      ? "top"
      : showSidebarTimeline || hasLeftSurfaceCustomSidebar
        ? "left"
        : showTopTimeline
          ? "top"
          : "default";

  return (
    <MainShellScaffold
      edgeToEdge={isOnboarding}
      mainSurfaceChrome={isOnboarding ? undefined : mainSurfaceChrome}
    >
      <MainShellBodyFrame>
        <ClassicMainBody />
      </MainShellBodyFrame>
      <ToastArea />
    </MainShellScaffold>
  );
}
