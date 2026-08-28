import * as stylex from "@stylexjs/stylex";
import { memo } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";

import { ClassicMainBody } from "./body";
import { resolveMainSurfaceChrome } from "./main-surface-chrome";
import { WindowsTitleBar } from "./windows-title-bar";

import { useShell } from "~/contexts/shell";
import { usesWindowsStyleTitleBar } from "~/shared/hooks/useWindowControlsGutter";
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

  if (!usesWindowsStyleTitleBar()) {
    return shell;
  }

  return (
    <div {...stylex.props(styles.root)}>
      <WindowsTitleBar />
      <div {...stylex.props(styles.content)}>{shell}</div>
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

const styles = stylex.create({
  content: {
    flex: "1",
    minHeight: 0,
  },
  root: {
    backgroundColor: colors.background,
    display: "flex",
    flexDirection: "column",
    height: "100%",
    minHeight: 0,
  },
});
