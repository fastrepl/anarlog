import {
  type CSSProperties,
  type WheelEvent as ReactWheelEvent,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@anlg/ui/components/ui/resizable";
import { cn } from "@anlg/utils";

import {
  createFixedLeftSidebarPanelConstraints,
  createLeftSidebarPanelConstraints,
  getMeasuredMainAreaWidthPx,
  LEFT_SIDEBAR_COLLAPSED_SIZE,
  LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
  LEFT_SIDEBAR_MAX_WIDTH_PX,
  LEFT_SIDEBAR_MIN_WIDTH_PX,
  panelSizesAreEqual,
  resizeLeftSidebarPanel,
} from "./left-sidebar-panel";
import { useMainAreaTopWindowDrag } from "./main-area-window-drag";
import { ClassicMainSidebar } from "./shell-sidebar";
import { SidebarTimelineChromeWithUpcomingMeeting } from "./sidebar-timeline-chrome";
import { SyncStatusIndicator } from "./sync-status";
import { ClassicMainTabContent } from "./tab-content";
import { useClassicMainShortcuts } from "./useShortcuts";

import { useShell } from "~/contexts/shell";
import { scrollElementByWheel } from "~/shared/dom/scroll-wheel";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  usesWindowsStyleTitleBar,
  useWindowControlsGutter,
} from "~/shared/hooks/useWindowControlsGutter";
import { getMainContentMinWidth } from "~/shared/main/layout-widths";
import { useOpenNoteDialog } from "~/shared/open-note-dialog";
import { useNewNote } from "~/shared/useNewNote";
import type { SidebarNoteFilter } from "~/sidebar/note-filter";
import {
  hasCustomSidebarTab,
  hasLeftSurfaceCustomSidebarTab,
  hasOwnSidebarHeaderTab,
} from "~/sidebar/use-custom-sidebar";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

type LeftSidebarSizeStyle = CSSProperties & {
  "--left-sidebar-panel-size": string;
  "--left-sidebar-panel-width": string;
};

export function ClassicMainBody({
  showSyncStatus = false,
}: {
  showSyncStatus?: boolean;
}) {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  useClassicMainShortcuts();
  const [leftSidebarPanelConstraints, setLeftSidebarPanelConstraints] =
    useState(createLeftSidebarPanelConstraints);
  const [leftSidebarPanelSize, setLeftSidebarPanelSize] = useState(
    leftSidebarPanelConstraints.defaultSize,
  );
  const bodyRootRef = useRef<HTMLDivElement>(null);
  const leftSidebarPanelRef = useRef<ImperativePanelHandle>(null);
  const leftSidebarPanelConstraintsRef = useRef(leftSidebarPanelConstraints);
  const leftSidebarPanelSizeRef = useRef(leftSidebarPanelSize);
  const lastExpandedLeftSidebarPanelSizeRef = useRef(leftSidebarPanelSize);
  const leftSidebarResizeDraggingRef = useRef(false);
  const leftSidebarDefaultSizeTrackingRef = useRef(true);
  const pendingLeftSidebarDefaultSizeRef = useRef<number | null>(null);
  const syncDefaultLeftSidebarPanelSizeRef = useRef<() => void>(() => {});
  const [showIgnoredTimelineEvents, setShowIgnoredTimelineEvents] =
    useState(false);
  const [noteFilter, setNoteFilter] = useState<SidebarNoteFilter>("mine");
  const showWindowControlsGutter = useWindowControlsGutter();
  const showSidebarToggleInBody = !usesWindowsStyleTitleBar();
  leftSidebarPanelConstraintsRef.current = leftSidebarPanelConstraints;

  const isOnboarding = currentTab?.type === "onboarding";
  const mainContentMinWidth = getMainContentMinWidth(currentTab);
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimelineChrome = !hasCustomSidebar && !isOnboarding;
  const canResizeLeftSidebarPanel = showSidebarTimelineChrome;
  const showSidebarTimeline = showSidebarTimelineChrome && leftsidebar.expanded;
  const showCollapsedSidebarTimelineChrome =
    showSidebarTimelineChrome && !leftsidebar.expanded;
  const mountLeftSidebarPanel = !isOnboarding;
  const showLeftSidebarPanel = mountLeftSidebarPanel && leftsidebar.expanded;
  const sidebarOwnsChromeRow = hasOwnSidebarHeaderTab(currentTab);
  const enableMainAreaTopDrag =
    showSidebarTimelineChrome || hasLeftSurfaceCustomSidebar;
  const mainAreaTopDrag = useMainAreaTopWindowDrag(enableMainAreaTopDrag);
  const currentSessionId =
    currentTab?.type === "sessions" ? currentTab.id : undefined;
  const createNewNote = useNewNote();
  const openNoteDialog = useOpenNoteDialog();
  const handleOpenNoteDialog = useCallback(() => {
    openNoteDialog.open();
  }, [openNoteDialog]);
  const applyLeftSidebarPanelSize = useCallback((size: number) => {
    const bodyRoot = bodyRootRef.current;
    if (!bodyRoot) {
      return;
    }

    bodyRoot.style.setProperty("--left-sidebar-panel-size", `${size}`);
    bodyRoot.style.setProperty("--left-sidebar-panel-width", `${size}%`);
  }, []);
  const commitLeftSidebarPanelSize = useCallback((size: number) => {
    setLeftSidebarPanelSize(size);
  }, []);
  const handlePanelLayout = useCallback(
    (sizes: number[]) => {
      if (!showLeftSidebarPanel) {
        return;
      }

      if (!canResizeLeftSidebarPanel) {
        leftSidebarResizeDraggingRef.current = false;
        pendingLeftSidebarDefaultSizeRef.current = null;
        return;
      }

      const sidebarSize = sizes[0];
      if (typeof sidebarSize === "number") {
        const pendingDefaultSize = pendingLeftSidebarDefaultSizeRef.current;
        if (
          pendingDefaultSize !== null &&
          !leftSidebarResizeDraggingRef.current
        ) {
          if (!panelSizesAreEqual(sidebarSize, pendingDefaultSize)) {
            return;
          }

          pendingLeftSidebarDefaultSizeRef.current = null;
        }

        if (
          !leftSidebarResizeDraggingRef.current &&
          !panelSizesAreEqual(
            sidebarSize,
            leftSidebarPanelConstraintsRef.current.defaultSize,
          )
        ) {
          leftSidebarDefaultSizeTrackingRef.current = false;
        }

        leftSidebarPanelSizeRef.current = sidebarSize;
        applyLeftSidebarPanelSize(sidebarSize);

        if (sidebarSize > LEFT_SIDEBAR_COLLAPSED_SIZE) {
          lastExpandedLeftSidebarPanelSizeRef.current = sidebarSize;
        }

        if (!leftSidebarResizeDraggingRef.current) {
          commitLeftSidebarPanelSize(
            sidebarSize > LEFT_SIDEBAR_COLLAPSED_SIZE
              ? sidebarSize
              : lastExpandedLeftSidebarPanelSizeRef.current,
          );
        }
      }
    },
    [
      applyLeftSidebarPanelSize,
      commitLeftSidebarPanelSize,
      canResizeLeftSidebarPanel,
      showLeftSidebarPanel,
    ],
  );
  const handleLeftSidebarResizeDragging = useCallback(
    (isDragging: boolean) => {
      leftSidebarResizeDraggingRef.current = isDragging;

      if (isDragging) {
        leftSidebarDefaultSizeTrackingRef.current = false;
        pendingLeftSidebarDefaultSizeRef.current = null;
      }

      if (!isDragging) {
        commitLeftSidebarPanelSize(
          leftSidebarPanelSizeRef.current > LEFT_SIDEBAR_COLLAPSED_SIZE
            ? leftSidebarPanelSizeRef.current
            : lastExpandedLeftSidebarPanelSizeRef.current,
        );
      }
    },
    [commitLeftSidebarPanelSize],
  );
  const restoreLeftSidebarPanelSize = useCallback(() => {
    const restoredSize = Math.max(
      lastExpandedLeftSidebarPanelSizeRef.current,
      leftSidebarPanelConstraints.minSize,
    );

    leftSidebarPanelSizeRef.current = restoredSize;
    lastExpandedLeftSidebarPanelSizeRef.current = restoredSize;
    commitLeftSidebarPanelSize(restoredSize);
    applyLeftSidebarPanelSize(restoredSize);
    resizeLeftSidebarPanel(leftSidebarPanelRef.current, restoredSize);
  }, [
    applyLeftSidebarPanelSize,
    commitLeftSidebarPanelSize,
    leftSidebarPanelConstraints.minSize,
  ]);
  const handleLeftSidebarPanelCollapse = useCallback(() => {
    leftSidebarResizeDraggingRef.current = false;
    restoreLeftSidebarPanelSize();
    leftsidebar.setExpanded(false);
  }, [leftsidebar.setExpanded, restoreLeftSidebarPanelSize]);
  const handleToggleLeftSidebar = useCallback(() => {
    leftSidebarResizeDraggingRef.current = false;

    if (!leftsidebar.expanded) {
      restoreLeftSidebarPanelSize();
      leftsidebar.toggleExpanded();
      return;
    }

    commitLeftSidebarPanelSize(
      leftSidebarPanelSizeRef.current > LEFT_SIDEBAR_COLLAPSED_SIZE
        ? leftSidebarPanelSizeRef.current
        : lastExpandedLeftSidebarPanelSizeRef.current,
    );
    leftsidebar.toggleExpanded();
  }, [
    commitLeftSidebarPanelSize,
    leftsidebar.expanded,
    leftsidebar.toggleExpanded,
    restoreLeftSidebarPanelSize,
  ]);
  const handleSidebarTimelineHeaderWheel = useCallback(
    (event: ReactWheelEvent<HTMLDivElement>) => {
      const scroller = event.currentTarget
        .closest("[data-left-sidebar-panel-content]")
        ?.querySelector<HTMLElement>("[data-sidebar-timeline-scroll]");

      scrollElementByWheel(scroller ?? null, event);
    },
    [],
  );
  const syncDefaultLeftSidebarPanelSize = useCallback(() => {
    if (!mountLeftSidebarPanel || leftSidebarResizeDraggingRef.current) {
      return;
    }

    if (!canResizeLeftSidebarPanel) {
      leftSidebarResizeDraggingRef.current = false;
      pendingLeftSidebarDefaultSizeRef.current = null;
    } else if (!leftSidebarDefaultSizeTrackingRef.current) {
      return;
    }

    const currentConstraints = leftSidebarPanelConstraintsRef.current;

    if (
      canResizeLeftSidebarPanel &&
      !panelSizesAreEqual(
        leftSidebarPanelSizeRef.current,
        currentConstraints.defaultSize,
      )
    ) {
      leftSidebarDefaultSizeTrackingRef.current = false;
      return;
    }

    const measuredWidth = getMeasuredMainAreaWidthPx(bodyRootRef.current);
    const nextConstraints = createLeftSidebarPanelConstraints(measuredWidth);

    if (
      panelSizesAreEqual(
        nextConstraints.defaultSize,
        currentConstraints.defaultSize,
      ) &&
      panelSizesAreEqual(nextConstraints.minSize, currentConstraints.minSize) &&
      panelSizesAreEqual(nextConstraints.maxSize, currentConstraints.maxSize)
    ) {
      return;
    }

    leftSidebarPanelConstraintsRef.current = nextConstraints;
    setLeftSidebarPanelConstraints(nextConstraints);
    leftSidebarPanelSizeRef.current = nextConstraints.defaultSize;
    lastExpandedLeftSidebarPanelSizeRef.current = nextConstraints.defaultSize;
    pendingLeftSidebarDefaultSizeRef.current = canResizeLeftSidebarPanel
      ? nextConstraints.defaultSize
      : null;
    commitLeftSidebarPanelSize(nextConstraints.defaultSize);
    applyLeftSidebarPanelSize(nextConstraints.defaultSize);

    window.requestAnimationFrame(() => {
      resizeLeftSidebarPanel(
        leftSidebarPanelRef.current,
        nextConstraints.defaultSize,
      );
    });
  }, [
    applyLeftSidebarPanelSize,
    canResizeLeftSidebarPanel,
    commitLeftSidebarPanelSize,
    mountLeftSidebarPanel,
  ]);
  syncDefaultLeftSidebarPanelSizeRef.current = syncDefaultLeftSidebarPanelSize;
  useMountEffect(() => {
    const bodyRoot = bodyRootRef.current;
    let syncFrame: number | null = null;

    const scheduleDefaultSizeSync = () => {
      if (syncFrame !== null) {
        window.cancelAnimationFrame(syncFrame);
      }

      syncFrame = window.requestAnimationFrame(() => {
        syncFrame = null;
        syncDefaultLeftSidebarPanelSizeRef.current();
      });
    };

    scheduleDefaultSizeSync();
    window.addEventListener("resize", scheduleDefaultSizeSync);

    const resizeObserver =
      typeof ResizeObserver !== "undefined" && bodyRoot
        ? new ResizeObserver(scheduleDefaultSizeSync)
        : null;
    if (resizeObserver && bodyRoot) {
      resizeObserver.observe(bodyRoot);
    }

    return () => {
      if (syncFrame !== null) {
        window.cancelAnimationFrame(syncFrame);
      }
      window.removeEventListener("resize", scheduleDefaultSizeSync);
      resizeObserver?.disconnect();
    };
  });
  const leftSidebarChromeStyle = useMemo(
    () =>
      ({
        width: canResizeLeftSidebarPanel
          ? "var(--left-sidebar-panel-width)"
          : LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
        minWidth: LEFT_SIDEBAR_MIN_WIDTH_PX,
        maxWidth: canResizeLeftSidebarPanel
          ? LEFT_SIDEBAR_MAX_WIDTH_PX
          : LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
      }) satisfies CSSProperties,
    [canResizeLeftSidebarPanel],
  );
  const leftSidebarPanelStyle = useMemo(() => {
    if (!leftsidebar.expanded) {
      return {
        flexGrow: 0,
        maxWidth: 0,
        minWidth: 0,
      } satisfies CSSProperties;
    }

    if (!canResizeLeftSidebarPanel) {
      return {
        flexBasis: LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
        flexGrow: 0,
        maxWidth: LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
        minWidth: LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
      } satisfies CSSProperties;
    }

    return {
      flexGrow: "var(--left-sidebar-panel-size)",
      maxWidth: LEFT_SIDEBAR_MAX_WIDTH_PX,
      minWidth: LEFT_SIDEBAR_MIN_WIDTH_PX,
    } satisfies CSSProperties;
  }, [canResizeLeftSidebarPanel, leftsidebar.expanded]);
  const leftSidebarPanelRenderConstraints = canResizeLeftSidebarPanel
    ? leftSidebarPanelConstraints
    : createFixedLeftSidebarPanelConstraints(
        leftSidebarPanelConstraints.defaultSize,
      );
  const renderedLeftSidebarPanelSize = leftSidebarResizeDraggingRef.current
    ? leftSidebarPanelSizeRef.current
    : leftSidebarPanelSize;
  const leftSidebarSizeStyle = {
    "--left-sidebar-panel-size": `${renderedLeftSidebarPanelSize}`,
    "--left-sidebar-panel-width": `${renderedLeftSidebarPanelSize}%`,
  } as LeftSidebarSizeStyle;
  const timelineHeader = showSidebarTimelineChrome ? (
    <div
      data-tauri-drag-region
      data-sidebar-timeline-header
      className={cn([
        "flex h-9 shrink-0 items-start pt-[9px] pr-1",
        showWindowControlsGutter ? "pl-[76px]" : "pl-2",
      ])}
      onWheelCapture={handleSidebarTimelineHeaderWheel}
    >
      {showSidebarTimeline ? (
        <SidebarTimelineChromeWithUpcomingMeeting
          currentSessionId={currentSessionId}
          noteFilter={noteFilter}
          sidebarExpanded
          showSidebarToggle={showSidebarToggleInBody}
          showIgnoredTimelineEvents={showIgnoredTimelineEvents}
          onNewNote={createNewNote}
          onNoteFilterChange={setNoteFilter}
          onSearch={handleOpenNoteDialog}
          onToggleSidebar={handleToggleLeftSidebar}
        />
      ) : null}
    </div>
  ) : null;

  return (
    <div
      ref={bodyRootRef}
      style={leftSidebarSizeStyle}
      className="relative flex h-full min-w-0 flex-1 flex-col"
    >
      {isOnboarding ||
      showSidebarTimeline ? null : showCollapsedSidebarTimelineChrome ? (
        <div
          data-tauri-drag-region
          data-left-sidebar-chrome
          style={leftSidebarChromeStyle}
          className={cn([
            "absolute top-0 z-40 h-12",
            "pointer-events-none left-1",
          ])}
        >
          <div
            data-tauri-drag-region
            className={cn([
              "flex h-full min-w-0 items-start pt-[9px] pr-1",
              showWindowControlsGutter ? "pl-[76px]" : "pl-2",
            ])}
          >
            <SidebarTimelineChromeWithUpcomingMeeting
              currentSessionId={currentSessionId}
              noteFilter={noteFilter}
              sidebarExpanded={false}
              showSidebarToggle={showSidebarToggleInBody}
              showIgnoredTimelineEvents={showIgnoredTimelineEvents}
              onNewNote={createNewNote}
              onNoteFilterChange={setNoteFilter}
              onSearch={handleOpenNoteDialog}
              onToggleSidebar={handleToggleLeftSidebar}
            />
          </div>
        </div>
      ) : hasLeftSurfaceCustomSidebar ? (
        <div
          data-tauri-drag-region
          data-left-sidebar-chrome
          style={leftSidebarChromeStyle}
          className={cn([
            "absolute top-0 left-0 z-40 h-10",
            sidebarOwnsChromeRow && "pointer-events-none",
          ])}
        />
      ) : (
        <div data-tauri-drag-region className="relative h-10 shrink-0">
          <div
            data-tauri-drag-region
            className={cn([
              "flex h-full min-w-0 items-start pt-1",
              showWindowControlsGutter ? "pl-[76px]" : "pl-2",
            ])}
          />
        </div>
      )}
      <ResizablePanelGroup
        autoSaveId={
          mountLeftSidebarPanel && canResizeLeftSidebarPanel
            ? "classic-main-sidebar"
            : undefined
        }
        dir="ltr"
        direction="horizontal"
        className="min-h-0 flex-1 overflow-hidden"
        onLayout={handlePanelLayout}
      >
        {mountLeftSidebarPanel ? (
          <>
            <ResizablePanel
              ref={leftSidebarPanelRef}
              id="classic-main-sidebar-left"
              order={1}
              collapsible
              collapsedSize={LEFT_SIDEBAR_COLLAPSED_SIZE}
              defaultSize={leftSidebarPanelRenderConstraints.defaultSize}
              minSize={leftSidebarPanelRenderConstraints.minSize}
              maxSize={leftSidebarPanelRenderConstraints.maxSize}
              onCollapse={handleLeftSidebarPanelCollapse}
              className={cn([
                "min-h-0 overflow-hidden",
                !leftsidebar.expanded && "pointer-events-none",
              ])}
              style={leftSidebarPanelStyle}
            >
              <div
                data-left-sidebar-panel-content
                aria-hidden={!leftsidebar.expanded}
                inert={!leftsidebar.expanded ? true : undefined}
                className={cn([
                  "h-full w-full transition-[opacity,transform] duration-200 ease-out",
                  leftsidebar.expanded
                    ? "translate-x-0 opacity-100"
                    : "-translate-x-3 opacity-0",
                ])}
              >
                <ClassicMainSidebar
                  noteFilter={noteFilter}
                  timelineHeader={timelineHeader}
                  showIgnoredTimelineEvents={showIgnoredTimelineEvents}
                  onShowIgnoredTimelineEventsChange={
                    setShowIgnoredTimelineEvents
                  }
                />
              </div>
            </ResizablePanel>
            <ResizableHandle
              className={cn([
                "z-10 !bg-transparent after:w-2",
                showLeftSidebarPanel && canResizeLeftSidebarPanel
                  ? "w-1"
                  : "pointer-events-none w-0 after:w-0",
              ])}
              onDragging={
                canResizeLeftSidebarPanel
                  ? handleLeftSidebarResizeDragging
                  : undefined
              }
            />
          </>
        ) : null}
        <ResizablePanel
          id="classic-main-content"
          order={2}
          className="min-h-0 flex-1 overflow-hidden"
          style={{ minWidth: mainContentMinWidth }}
        >
          <div
            data-main-content-panel
            className="relative h-full min-h-0 min-w-0 flex-1 overflow-auto"
            onClickCapture={mainAreaTopDrag.onClickCapture}
            onDoubleClickCapture={mainAreaTopDrag.onDoubleClickCapture}
            onPointerCancel={mainAreaTopDrag.onPointerEnd}
            onPointerDown={mainAreaTopDrag.onPointerDown}
            onPointerMove={mainAreaTopDrag.onPointerMove}
            onPointerUp={mainAreaTopDrag.onPointerEnd}
          >
            {currentTab ? (
              <ClassicMainTabContent
                key={uniqueIdfromTab(currentTab)}
                tab={currentTab as Tab}
              />
            ) : null}
            {showSyncStatus ? <SyncStatusIndicator /> : null}
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
