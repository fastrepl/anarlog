import { platform } from "@tauri-apps/plugin-os";
import { memo } from "react";

import { ClassicMainBody } from "./body";
import { resolveMainSurfaceChrome } from "./main-surface-chrome";
import { WindowsTitleBar } from "./windows-title-bar";

import { useShell } from "~/contexts/shell";
import { MainShellBodyFrame, MainShellScaffold } from "~/shared/main";
import { ToastNotifications } from "~/sidebar/toast";
import {
  hasCustomSidebarTab,
  hasLeftSurfaceCustomSidebarTab,
} from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainShellFrame() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);

  const isOnboarding = currentTab?.type === "onboarding";
  const isChangelog = currentTab?.type === "changelog";
  const showSyncStatus =
    currentTab?.type === "empty" || currentTab?.type === "sessions";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimelineChrome = !hasCustomSidebar && !isOnboarding;
  const showSidebarTimeline = showSidebarTimelineChrome && leftsidebar.expanded;
  const mainSurfaceChrome = resolveMainSurfaceChrome({
    hasLeftSurfaceCustomSidebar,
    isChangelog,
    leftSidebarExpanded: leftsidebar.expanded,
    showSidebarTimeline,
    showSidebarTimelineChrome,
  });

  const shell = (
    <MainShellScaffold
      edgeToEdge={isOnboarding}
      mainSurfaceChrome={isOnboarding ? undefined : mainSurfaceChrome}
    >
      <ClassicMainBodyHost showSyncStatus={showSyncStatus} />
      <ToastNotifications />
    </MainShellScaffold>
  );

  if (platform() !== "windows") {
    return shell;
  }

  return (
    <div className="bg-background flex h-full min-h-0 flex-col">
      <WindowsTitleBar />
      <div className="min-h-0 flex-1">{shell}</div>
    </div>
  );
}

const ClassicMainBodyHost = memo(function ClassicMainBodyHost({
  showSyncStatus,
}: {
  showSyncStatus: boolean;
}) {
  return (
    <MainShellBodyFrame>
      <ClassicMainBody showSyncStatus={showSyncStatus} />
    </MainShellBodyFrame>
  );
});
