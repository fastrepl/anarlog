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
import { NOTE_SURFACE_MIN_WIDTH_PX } from "~/shared/main/layout-widths";
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
  const [leftSidebarPanelConstraints] = useState(
    createLeftSidebarPanelConstraints,
  );
  const [leftSidebarPanelSize, setLeftSidebarPanelSize] = useState(
    leftSidebarPanelConstraints.defaultSize,
  );
  const bodyRootRef = useRef<HTMLDivElement>(null);
  const leftSidebarPanelRef = useRef<ImperativePanelHandle>(null);
  const leftSidebarPanelSizeRef = useRef(leftSidebarPanelSize);
  const lastExpandedLeftSidebarPanelSizeRef = useRef(leftSidebarPanelSize);
  const leftSidebarResizeDraggingRef = useRef(false);
  const [showIgnoredTimelineEvents, setShowIgnoredTimelineEvents] =
    useState(false);
  const [showDevtoolsPanelButton, setShowDevtoolsPanelButton] = useState(false);
  const [devtoolsPanelOpen, setDevtoolsPanelOpen] = useState(false);

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
  const isSessionTab = currentTab?.type === "sessions";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimelineChrome = !hasCustomSidebar && !isOnboarding;
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

      const sidebarSize = sizes[0];
      if (typeof sidebarSize === "number") {
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
      showLeftSidebarPanel,
    ],
  );
  const handleLeftSidebarResizeDragging = useCallback(
    (isDragging: boolean) => {
      leftSidebarResizeDraggingRef.current = isDragging;
      setLeftSidebarResizing(isDragging);

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
        width: "var(--left-sidebar-panel-width)",
        minWidth: LEFT_SIDEBAR_MIN_WIDTH_PX,
        maxWidth: LEFT_SIDEBAR_MAX_WIDTH_PX,
      }) satisfies CSSProperties,
    [],
  );
  const leftSidebarPanelStyle = useMemo(
    () =>
      ({
        flexGrow: leftsidebar.expanded ? "var(--left-sidebar-panel-size)" : 0,
        maxWidth: leftsidebar.expanded ? LEFT_SIDEBAR_MAX_WIDTH_PX : 0,
        minWidth: leftsidebar.expanded ? LEFT_SIDEBAR_MIN_WIDTH_PX : 0,
        transition:
          leftSidebarResizing && leftsidebar.expanded
            ? undefined
            : [
                "flex-grow 180ms ease-out",
                "max-width 180ms ease-out",
                "min-width 180ms ease-out",
              ].join(", "),
      }) satisfies CSSProperties,
    [leftSidebarResizing, leftsidebar.expanded],
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
        autoSaveId={mountLeftSidebarPanel ? "classic-main-sidebar" : undefined}
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
              defaultSize={leftSidebarPanelConstraints.defaultSize}
              minSize={leftSidebarPanelConstraints.minSize}
              maxSize={leftSidebarPanelConstraints.maxSize}
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
                showLeftSidebarPanel
                  ? "w-1"
                  : "pointer-events-none w-0 after:w-0",
              ])}
              onDragging={handleLeftSidebarResizeDragging}
            />
          </>
        ) : null}
        <ResizablePanel
          id="classic-main-content"
          order={2}
          className="min-h-0 flex-1 overflow-hidden"
          style={{
            minWidth: isSessionTab ? NOTE_SURFACE_MIN_WIDTH_PX : undefined,
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

function createLeftSidebarPanelConstraints() {
  const containerWidthPx = Math.max(
    getInitialMainAreaWidthPx(),
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
