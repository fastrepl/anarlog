import * as stylex from "@stylexjs/stylex";
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
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
      {...stylex.props(
        styles.timelineHeader,
        showWindowControlsGutter
          ? styles.windowControlsGutter
          : styles.defaultGutter,
      )}
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
      {...mergeStyleXProps(styles.root, undefined, leftSidebarSizeStyle)}
    >
      {isOnboarding ||
      showSidebarTimeline ? null : showCollapsedSidebarTimelineChrome ? (
        <div
          data-tauri-drag-region
          data-left-sidebar-chrome
          {...mergeStyleXProps(
            styles.collapsedSidebarChrome,
            undefined,
            leftSidebarChromeStyle,
          )}
        >
          <div
            data-tauri-drag-region
            {...stylex.props(
              styles.collapsedSidebarChromeContent,
              showWindowControlsGutter
                ? styles.windowControlsGutter
                : styles.defaultGutter,
            )}
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
          {...mergeStyleXProps(
            [
              styles.customSidebarChrome,
              sidebarOwnsChromeRow && styles.pointerEventsNone,
            ],
            undefined,
            leftSidebarChromeStyle,
          )}
        />
      ) : (
        <div data-tauri-drag-region {...stylex.props(styles.dragHeader)}>
          <div
            data-tauri-drag-region
            {...stylex.props(
              styles.dragHeaderContent,
              showWindowControlsGutter
                ? styles.windowControlsGutter
                : styles.defaultGutter,
            )}
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
        sx={styles.panelGroup}
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
              sx={[
                styles.panel,
                !leftsidebar.expanded && styles.pointerEventsNone,
              ]}
              style={leftSidebarPanelStyle}
            >
              <div
                data-left-sidebar-panel-content
                aria-hidden={!leftsidebar.expanded}
                inert={!leftsidebar.expanded ? true : undefined}
                {...stylex.props(
                  styles.sidebarContent,
                  leftsidebar.expanded
                    ? styles.sidebarContentExpanded
                    : styles.sidebarContentCollapsed,
                )}
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
              sx={[
                styles.resizeHandle,
                showLeftSidebarPanel && canResizeLeftSidebarPanel
                  ? styles.resizeHandleEnabled
                  : styles.resizeHandleDisabled,
              ]}
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
          sx={styles.mainPanel}
          style={{ minWidth: mainContentMinWidth }}
        >
          <div
            data-main-content-panel
            {...stylex.props(styles.mainContent)}
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

const styles = stylex.create({
  collapsedSidebarChrome: {
    height: "3rem",
    left: "0.25rem",
    pointerEvents: "none",
    position: "absolute",
    top: 0,
    zIndex: 40,
  },
  collapsedSidebarChromeContent: {
    alignItems: "flex-start",
    display: "flex",
    height: "100%",
    minWidth: 0,
    paddingRight: "0.25rem",
    paddingTop: "9px",
  },
  customSidebarChrome: {
    height: "2.5rem",
    left: 0,
    position: "absolute",
    top: 0,
    zIndex: 40,
  },
  defaultGutter: {
    paddingLeft: "0.5rem",
  },
  dragHeader: {
    flexShrink: 0,
    height: "2.5rem",
    position: "relative",
  },
  dragHeaderContent: {
    alignItems: "flex-start",
    display: "flex",
    height: "100%",
    minWidth: 0,
    paddingTop: "0.25rem",
  },
  mainContent: {
    flex: "1",
    height: "100%",
    minHeight: 0,
    minWidth: 0,
    overflow: "auto",
    position: "relative",
  },
  mainPanel: {
    flex: "1",
    minHeight: 0,
    overflow: "hidden",
  },
  panel: {
    minHeight: 0,
    overflow: "hidden",
  },
  panelGroup: {
    flex: "1",
    minHeight: 0,
    overflow: "hidden",
  },
  pointerEventsNone: {
    pointerEvents: "none",
  },
  resizeHandle: {
    "::after": {
      width: "0.5rem",
    },
    backgroundColor: "transparent",
    zIndex: 10,
  },
  resizeHandleDisabled: {
    "::after": {
      width: 0,
    },
    pointerEvents: "none",
    width: 0,
  },
  resizeHandleEnabled: {
    width: "0.25rem",
  },
  root: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
    minWidth: 0,
    position: "relative",
  },
  sidebarContent: {
    height: "100%",
    transitionDuration: "200ms",
    transitionProperty: "opacity, transform",
    transitionTimingFunction: "cubic-bezier(0, 0, 0.2, 1)",
    width: "100%",
  },
  sidebarContentCollapsed: {
    opacity: 0,
    transform: "translateX(-0.75rem)",
  },
  sidebarContentExpanded: {
    opacity: 1,
    transform: "translateX(0)",
  },
  timelineHeader: {
    alignItems: "flex-start",
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    paddingRight: "0.25rem",
    paddingTop: "9px",
  },
  windowControlsGutter: {
    paddingLeft: "76px",
  },
});

export { styles as classicMainBodyStyles };
