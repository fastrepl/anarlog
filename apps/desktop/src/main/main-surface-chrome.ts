import { type MainSurfaceChrome } from "~/shared/main";

export function resolveMainSurfaceChrome({
  hasLeftSurfaceCustomSidebar,
  isChangelog,
  leftSidebarExpanded,
  showSidebarTimeline,
  showSidebarTimelineChrome,
  showTopTimeline,
}: {
  hasLeftSurfaceCustomSidebar: boolean;
  isChangelog: boolean;
  leftSidebarExpanded: boolean;
  showSidebarTimeline: boolean;
  showSidebarTimelineChrome: boolean;
  showTopTimeline: boolean;
}): MainSurfaceChrome {
  if (showSidebarTimelineChrome && !leftSidebarExpanded) {
    return "top-borderless";
  }

  if (isChangelog && !showSidebarTimeline) {
    return "top";
  }

  if (showSidebarTimeline || hasLeftSurfaceCustomSidebar) {
    return "left";
  }

  if (showTopTimeline) {
    return "top";
  }

  return "default";
}
