import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeftIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  SearchIcon,
  SquarePenIcon,
  WrenchIcon,
} from "lucide-react";
import {
  type CSSProperties,
  type MouseEvent,
  type PointerEvent,
  type WheelEvent,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  commands as windowsCommands,
  events as windowsEvents,
} from "@hypr/plugin-windows";
import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { cn } from "@hypr/utils";

import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabContent } from "./tab-content";
import {
  type DesktopUpdateControl,
  SidebarTimelineUpdateButton,
  useDesktopUpdateControl,
} from "./update-banner";
import { useClassicMainTabsShortcuts } from "./useTabsShortcuts";

import { useShell } from "~/contexts/shell";
import { scrollElementByWheel } from "~/shared/dom/scroll-wheel";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  NOTE_SURFACE_MIN_WIDTH_PX,
  usesNoteSurfaceMinWidth,
} from "~/shared/main/layout-widths";
import { useOpenNoteDialog } from "~/shared/open-note-dialog";
import { useNewNote } from "~/shared/useNewNote";
import { useSidebarUpcomingMeetingStatus } from "~/sidebar/timeline/upcoming-meeting";
import {
  hasCustomSidebarTab,
  hasLeftSurfaceCustomSidebarTab,
} from "~/sidebar/use-custom-sidebar";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { commands } from "~/types/tauri.gen";

const MAIN_AREA_TOP_DRAG_HEIGHT_PX = 48;
const MAIN_AREA_WINDOW_DRAG_THRESHOLD_PX = 5;
const LEFT_SIDEBAR_DEFAULT_WIDTH_PX = 200;
const LEFT_SIDEBAR_MIN_WIDTH_PX = 200;
const LEFT_SIDEBAR_MAX_WIDTH_PX = 360;
const LEFT_SIDEBAR_COLLAPSED_SIZE = 0;
const LEFT_SIDEBAR_FALLBACK_CONTAINER_WIDTH_PX = 1000;
const LEFT_SIDEBAR_PANEL_SIZE_EPSILON = 0.01;

type MainAreaWindowDragStart = {
  pointerId: number;
  clientX: number;
  clientY: number;
  dragging: boolean;
};
type LeftSidebarSizeStyle = CSSProperties & {
  "--left-sidebar-panel-size": string;
  "--left-sidebar-panel-width": string;
};

export function ClassicMainBody() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const { runEscapeShortcut } = useClassicMainTabsShortcuts();
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
  const [showDevtoolsPanelButton, setShowDevtoolsPanelButton] = useState(false);
  const [devtoolsPanelOpen, setDevtoolsPanelOpen] = useState(false);
  leftSidebarPanelConstraintsRef.current = leftSidebarPanelConstraints;

  useMountEffect(() => {
    let cancelled = false;
    let unlistenDevtoolsAction: (() => void) | undefined;

    const syncDevtoolsPanelButton = async () => {
      const enabled = await commands.showDevtool().catch((error) => {
        console.error("Failed to resolve devtools availability:", error);
        return false;
      });

      if (cancelled) {
        return;
      }

      setShowDevtoolsPanelButton(enabled);

      if (!enabled) {
        return;
      }

      windowsEvents.devtoolsPanelAction
        .listen(({ payload }) => {
          if (payload.action === "panel:opened") {
            setDevtoolsPanelOpen(true);
          }
          if (payload.action === "panel:closed") {
            setDevtoolsPanelOpen(false);
          }
        })
        .then((unlisten) => {
          if (cancelled) {
            unlisten();
            return;
          }

          unlistenDevtoolsAction = unlisten;
        });
    };

    void syncDevtoolsPanelButton();

    return () => {
      cancelled = true;
      unlistenDevtoolsAction?.();
    };
  });

  const isOnboarding = currentTab?.type === "onboarding";
  const reserveNoteSurfaceMinWidth = usesNoteSurfaceMinWidth(currentTab);
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimelineChrome = !hasCustomSidebar && !isOnboarding;
  const canResizeLeftSidebarPanel = showSidebarTimelineChrome;
  const showSidebarTimeline = showSidebarTimelineChrome && leftsidebar.expanded;
  const mountLeftSidebarPanel = !isOnboarding;
  const showLeftSidebarPanel = mountLeftSidebarPanel && leftsidebar.expanded;
  const showLeftSurfaceChromeBack = hasLeftSurfaceCustomSidebar;
  const enableMainAreaTopDrag =
    showSidebarTimelineChrome || hasLeftSurfaceCustomSidebar;
  const mainAreaTopDrag = useMainAreaTopWindowDrag(enableMainAreaTopDrag);
  const update = useDesktopUpdateControl();
  const upcomingMeetingStatus = useSidebarUpcomingMeetingStatus({
    showIgnored: showIgnoredTimelineEvents,
  });
  const [leftSidebarResizing, setLeftSidebarResizing] = useState(false);
  const hasUpcomingMeetingBadge = upcomingMeetingStatus
    ? currentTab?.type !== "sessions" ||
      upcomingMeetingStatus.itemKey !== `session-${currentTab.id}`
    : false;
  const createNewNote = useNewNote();
  const openNoteDialog = useOpenNoteDialog();
  const handleOpenNoteDialog = useCallback(() => {
    openNoteDialog.open();
  }, [openNoteDialog]);
  const handleOpenDevtoolsPanel = useCallback(async () => {
    const result = await windowsCommands.devtoolsPanelShow();

    if (result.status === "error") {
      console.error("Failed to show devtools panel:", result.error);
    }
  }, []);
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
        setLeftSidebarResizing(false);
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
      setLeftSidebarResizing(isDragging);

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
    setLeftSidebarResizing(false);
    restoreLeftSidebarPanelSize();
    leftsidebar.setExpanded(false);
  }, [leftsidebar.setExpanded, restoreLeftSidebarPanelSize]);
  const handleToggleLeftSidebar = useCallback(() => {
    leftSidebarResizeDraggingRef.current = false;
    setLeftSidebarResizing(false);

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
  const syncDefaultLeftSidebarPanelSize = useCallback(() => {
    if (!mountLeftSidebarPanel || leftSidebarResizeDraggingRef.current) {
      return;
    }

    if (!canResizeLeftSidebarPanel) {
      leftSidebarResizeDraggingRef.current = false;
      setLeftSidebarResizing(false);
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
  const handleLeftSidebarChromeWheel = useCallback(
    (event: WheelEvent<HTMLDivElement>) => {
      if (!showSidebarTimeline) {
        return;
      }

      const timelineScroller =
        event.currentTarget.parentElement?.querySelector<HTMLElement>(
          "[data-sidebar-timeline-scroll]",
        ) ?? null;

      scrollElementByWheel(timelineScroller, event);
    },
    [showSidebarTimeline],
  );
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
        transition:
          leftSidebarResizing && leftsidebar.expanded
            ? undefined
            : [
                "flex-grow 180ms ease-out",
                "max-width 180ms ease-out",
                "min-width 180ms ease-out",
              ].join(", "),
      } satisfies CSSProperties;
    }

    if (!canResizeLeftSidebarPanel) {
      return {
        flexBasis: LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
        flexGrow: 0,
        maxWidth: LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
        minWidth: LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
        transition:
          leftSidebarResizing && leftsidebar.expanded
            ? undefined
            : [
                "flex-grow 180ms ease-out",
                "max-width 180ms ease-out",
                "min-width 180ms ease-out",
              ].join(", "),
      } satisfies CSSProperties;
    }

    return {
      flexGrow: "var(--left-sidebar-panel-size)",
      maxWidth: LEFT_SIDEBAR_MAX_WIDTH_PX,
      minWidth: LEFT_SIDEBAR_MIN_WIDTH_PX,
      transition:
        leftSidebarResizing && leftsidebar.expanded
          ? undefined
          : [
              "flex-grow 180ms ease-out",
              "max-width 180ms ease-out",
              "min-width 180ms ease-out",
            ].join(", "),
    } satisfies CSSProperties;
  }, [canResizeLeftSidebarPanel, leftSidebarResizing, leftsidebar.expanded]);
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

  return (
    <div
      ref={bodyRootRef}
      style={leftSidebarSizeStyle}
      className="relative flex h-full min-w-0 flex-1 flex-col"
    >
      {isOnboarding ? null : showSidebarTimelineChrome ? (
        <div
          data-tauri-drag-region
          data-left-sidebar-chrome
          style={leftSidebarChromeStyle}
          onWheel={handleLeftSidebarChromeWheel}
          className={cn([
            "absolute top-0 z-40 h-12",
            leftsidebar.expanded ? "left-0" : "left-1",
            !leftsidebar.expanded && "pointer-events-none",
          ])}
        >
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-[9px] pr-1 pl-[76px]"
          >
            <SidebarTimelineChrome
              sidebarExpanded={leftsidebar.expanded}
              showDevtoolsPanelButton={showDevtoolsPanelButton}
              devtoolsPanelOpen={devtoolsPanelOpen}
              onNewNote={createNewNote}
              onSearch={handleOpenNoteDialog}
              onOpenDevtools={handleOpenDevtoolsPanel}
              onToggleSidebar={handleToggleLeftSidebar}
              hasUpcomingMeeting={hasUpcomingMeetingBadge}
              update={update}
            />
          </div>
        </div>
      ) : hasLeftSurfaceCustomSidebar ? (
        <div
          data-tauri-drag-region
          data-left-sidebar-chrome
          style={leftSidebarChromeStyle}
          className="absolute top-0 left-0 z-40 h-10"
        />
      ) : (
        <div data-tauri-drag-region className="relative h-10 shrink-0">
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-1 pl-[76px]"
          />
        </div>
      )}
      {showLeftSurfaceChromeBack ? (
        <div
          data-tauri-drag-region
          data-left-sidebar-chrome
          style={leftSidebarChromeStyle}
          className="absolute top-0 left-0 z-50 h-12"
        >
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-[9px] pl-[76px]"
          >
            <LeftSurfaceChromeButton
              ariaLabel="Go back"
              onClick={runEscapeShortcut}
            >
              <ArrowLeftIcon size={16} />
            </LeftSurfaceChromeButton>
          </div>
        </div>
      ) : null}
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
                  forceMount
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
          style={{
            minWidth: reserveNoteSurfaceMinWidth
              ? NOTE_SURFACE_MIN_WIDTH_PX
              : undefined,
          }}
        >
          <div
            data-main-content-panel
            className="h-full min-h-0 min-w-0 flex-1 overflow-auto"
            onClickCapture={mainAreaTopDrag.onClickCapture}
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
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

function createLeftSidebarPanelConstraints(widthPx?: number) {
  const containerWidthPx = Math.max(
    widthPx ?? getInitialMainAreaWidthPx(),
    LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
  );
  const minSize = percentageFromPixels(
    LEFT_SIDEBAR_MIN_WIDTH_PX,
    containerWidthPx,
  );

  return {
    defaultSize: percentageFromPixels(
      LEFT_SIDEBAR_DEFAULT_WIDTH_PX,
      containerWidthPx,
    ),
    minSize,
    maxSize: Math.max(
      minSize,
      percentageFromPixels(LEFT_SIDEBAR_MAX_WIDTH_PX, containerWidthPx),
    ),
  };
}

function createFixedLeftSidebarPanelConstraints(defaultSize: number) {
  return {
    defaultSize,
    minSize: defaultSize,
    maxSize: defaultSize,
  };
}

function getMeasuredMainAreaWidthPx(element: HTMLElement | null) {
  const measuredWidth = element?.getBoundingClientRect().width ?? 0;

  return measuredWidth > 0 ? measuredWidth : getInitialMainAreaWidthPx();
}

function getInitialMainAreaWidthPx() {
  if (typeof window === "undefined") {
    return LEFT_SIDEBAR_FALLBACK_CONTAINER_WIDTH_PX;
  }

  return (
    window.innerWidth ||
    document.documentElement.clientWidth ||
    LEFT_SIDEBAR_FALLBACK_CONTAINER_WIDTH_PX
  );
}

function percentageFromPixels(widthPx: number, containerWidthPx: number) {
  return Math.min((widthPx / containerWidthPx) * 100, 100);
}

function panelSizesAreEqual(left: number, right: number) {
  return Math.abs(left - right) < LEFT_SIDEBAR_PANEL_SIZE_EPSILON;
}

function resizeLeftSidebarPanel(
  panel: ImperativePanelHandle | null,
  size: number,
) {
  if (!panel) {
    return;
  }

  try {
    panel.resize(size);
  } catch {
    window.requestAnimationFrame(() => {
      try {
        panel.resize(size);
      } catch {
        // The panel can be layoutless while hidden; the CSS variables still restore visual width on reopen.
      }
    });
  }
}

function useMainAreaTopWindowDrag(enabled: boolean) {
  const windowDragStartRef = useRef<MainAreaWindowDragStart | null>(null);
  const suppressNextClickRef = useRef(false);

  const handlePointerDown = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      suppressNextClickRef.current = false;

      if (
        !enabled ||
        event.button !== 0 ||
        isInteractiveMainAreaDragTarget(event.target) ||
        !isWithinMainAreaTopDragRegion(event)
      ) {
        windowDragStartRef.current = null;
        return;
      }

      windowDragStartRef.current = {
        pointerId: event.pointerId,
        clientX: event.clientX,
        clientY: event.clientY,
        dragging: false,
      };
    },
    [enabled],
  );

  const handlePointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const dragStart = windowDragStartRef.current;

      if (
        !dragStart ||
        dragStart.dragging ||
        dragStart.pointerId !== event.pointerId ||
        !isMainAreaWindowDrag(dragStart, event)
      ) {
        return;
      }

      dragStart.dragging = true;
      suppressNextClickRef.current = true;
      event.preventDefault();

      if (isTauri()) {
        void getCurrentWindow()
          .startDragging()
          .catch(() => {});
      }
    },
    [],
  );

  const handlePointerEnd = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const dragStart = windowDragStartRef.current;

      if (!dragStart || dragStart.pointerId !== event.pointerId) {
        return;
      }

      windowDragStartRef.current = null;
    },
    [],
  );

  const handleClickCapture = useCallback(
    (event: MouseEvent<HTMLDivElement>) => {
      if (!suppressNextClickRef.current) {
        return;
      }

      suppressNextClickRef.current = false;
      event.preventDefault();
      event.stopPropagation();
    },
    [],
  );

  return {
    onClickCapture: handleClickCapture,
    onPointerDown: handlePointerDown,
    onPointerEnd: handlePointerEnd,
    onPointerMove: handlePointerMove,
  };
}

function isWithinMainAreaTopDragRegion(
  event: PointerEvent<HTMLDivElement>,
): boolean {
  const rect = event.currentTarget.getBoundingClientRect();
  const offsetY = event.clientY - rect.top;

  return offsetY >= 0 && offsetY < MAIN_AREA_TOP_DRAG_HEIGHT_PX;
}

function isInteractiveMainAreaDragTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }

  return Boolean(
    target.closest(
      [
        "a",
        "button",
        "input",
        "select",
        "textarea",
        "[contenteditable='true']",
        "[role='button']",
        "[role='textbox']",
      ].join(","),
    ),
  );
}

function isMainAreaWindowDrag(
  start: { clientX: number; clientY: number },
  current: { clientX: number; clientY: number },
): boolean {
  const deltaX = current.clientX - start.clientX;
  const deltaY = current.clientY - start.clientY;

  return (
    deltaX * deltaX + deltaY * deltaY >=
    MAIN_AREA_WINDOW_DRAG_THRESHOLD_PX * MAIN_AREA_WINDOW_DRAG_THRESHOLD_PX
  );
}

function SidebarTimelineChrome({
  devtoolsPanelOpen,
  hasUpcomingMeeting,
  onNewNote,
  onOpenDevtools,
  onSearch,
  onToggleSidebar,
  sidebarExpanded,
  showDevtoolsPanelButton,
  update,
}: {
  devtoolsPanelOpen: boolean;
  hasUpcomingMeeting: boolean;
  onNewNote: () => void;
  onOpenDevtools: () => void;
  onSearch: () => void;
  onToggleSidebar: () => void;
  sidebarExpanded: boolean;
  showDevtoolsPanelButton: boolean;
  update: DesktopUpdateControl;
}) {
  const updateVisible = Boolean(update.status && update.version);
  const showUpdateButton = sidebarExpanded && updateVisible;
  const collapsedBadge = !sidebarExpanded
    ? hasUpcomingMeeting
      ? "upcomingMeeting"
      : updateVisible
        ? "update"
        : null
    : null;

  return (
    <div
      data-tauri-drag-region
      className="flex w-full items-center justify-between"
    >
      <div data-tauri-drag-region className="flex items-center gap-0">
        <LeftSurfaceChromeButton
          ariaLabel={sidebarExpanded ? "Hide sidebar" : "Show sidebar"}
          badge={collapsedBadge}
          onClick={onToggleSidebar}
        >
          {sidebarExpanded ? (
            <PanelLeftCloseIcon size={16} />
          ) : (
            <PanelLeftOpenIcon size={16} />
          )}
        </LeftSurfaceChromeButton>
        {sidebarExpanded ? (
          <>
            <LeftSurfaceChromeButton ariaLabel="Search" onClick={onSearch}>
              <SearchIcon size={15} />
            </LeftSurfaceChromeButton>
            <LeftSurfaceChromeButton ariaLabel="New note" onClick={onNewNote}>
              <SquarePenIcon size={15} />
            </LeftSurfaceChromeButton>
            {showDevtoolsPanelButton && !devtoolsPanelOpen ? (
              <LeftSurfaceChromeButton
                ariaLabel="Show devtools panel"
                onClick={onOpenDevtools}
              >
                <WrenchIcon size={15} />
              </LeftSurfaceChromeButton>
            ) : null}
          </>
        ) : null}
      </div>
      {showUpdateButton ? (
        <SidebarTimelineUpdateButton update={update} />
      ) : null}
    </div>
  );
}

type LeftSurfaceChromeBadge = "update" | "upcomingMeeting";

function LeftSurfaceChromeButton({
  ariaLabel,
  badge = null,
  children,
  disabled = false,
  onClick,
}: {
  ariaLabel: string;
  badge?: LeftSurfaceChromeBadge | null;
  children: React.ReactNode;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      data-tauri-drag-region="false"
      disabled={disabled}
      className={cn([
        "pointer-events-auto relative flex size-7 items-center justify-center rounded-full",
        "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        "disabled:text-muted-foreground/70 disabled:hover:text-muted-foreground/70 disabled:hover:bg-transparent",
      ])}
      onClick={onClick}
    >
      {children}
      {badge ? (
        <span
          aria-hidden="true"
          data-testid={
            badge === "upcomingMeeting"
              ? "collapsed-sidebar-upcoming-meeting-badge"
              : "collapsed-sidebar-update-badge"
          }
          className={cn([
            "ring-background pointer-events-none absolute top-1 right-1 size-1.5 rounded-full ring-2",
            badge === "upcomingMeeting" ? "bg-red-500" : "bg-blue-500",
          ])}
        />
      ) : null}
    </button>
  );
}
