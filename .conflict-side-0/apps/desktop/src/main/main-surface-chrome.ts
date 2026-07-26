import { type MainSurfaceChrome } from "~/shared/main";

export function resolveMainSurfaceChrome({
  hasLeftSurfaceCustomSidebar,
  isChangelog,
  leftSidebarExpanded,
  showSidebarTimeline,
  showSidebarTimelineChrome,
}: {
  hasLeftSurfaceCustomSidebar: boolean;
  isChangelog: boolean;
  leftSidebarExpanded: boolean;
  showSidebarTimeline: boolean;
  showSidebarTimelineChrome: boolean;
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

  return "default";
}
