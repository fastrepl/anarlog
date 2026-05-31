import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowLeftIcon, ArrowRightIcon } from "lucide-react";
import { type MouseEvent, type PointerEvent, useCallback, useRef } from "react";

import { cn } from "@hypr/utils";

import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabContent } from "./tab-content";
import { TopMeetingTimeline } from "./top-meeting-timeline";
import { useClassicMainTabsShortcuts } from "./useTabsShortcuts";

import { useShell } from "~/contexts/shell";
import { useConfigValue } from "~/shared/config";
import {
  hasCustomSidebarTab,
  hasLeftSurfaceCustomSidebarTab,
} from "~/sidebar/use-custom-sidebar";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

const MAIN_AREA_TOP_DRAG_HEIGHT_PX = 48;
const MAIN_AREA_WINDOW_DRAG_THRESHOLD_PX = 5;

type MainAreaWindowDragStart = {
  pointerId: number;
  clientX: number;
  clientY: number;
  dragging: boolean;
};

export function ClassicMainBody() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const goBack = useTabs((state) => state.goBack);
  const goNext = useTabs((state) => state.goNext);
  const canGoBack = useTabs((state) => state.canGoBack);
  const canGoNext = useTabs((state) => state.canGoNext);
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const { runEscapeShortcut } = useClassicMainTabsShortcuts();

  const isOnboarding = currentTab?.type === "onboarding";
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
  const showLeftSurfaceChromeBack = hasLeftSurfaceCustomSidebar;
  const enableMainAreaTopDrag =
    showSidebarTimeline || hasLeftSurfaceCustomSidebar;
  const mainAreaTopDrag = useMainAreaTopWindowDrag(enableMainAreaTopDrag);

  return (
    <div className="relative flex h-full min-w-0 flex-1 flex-col">
      {isOnboarding ? null : showSidebarTimeline ? (
        <div
          data-tauri-drag-region
          className="absolute top-0 left-0 z-40 h-12 w-[200px]"
        >
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-[9px] pl-[76px]"
          >
            <SidebarTimelineChrome
              canGoBack={canGoBack}
              canGoNext={canGoNext}
              onBack={goBack}
              onForward={goNext}
            />
          </div>
        </div>
      ) : hasLeftSurfaceCustomSidebar ? (
        <div
          data-tauri-drag-region
          className="absolute top-0 left-0 z-40 h-10 w-[200px]"
        />
      ) : (
        <div
          data-tauri-drag-region
          className={cn([
            "relative shrink-0",
            showTopTimeline ? "h-12" : "h-10",
          ])}
        >
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-1 pl-[76px]"
          >
            {showTopTimeline ? (
              <div className="min-w-0 flex-1">
                <TopMeetingTimeline currentTab={currentTab} />
              </div>
            ) : null}
          </div>
        </div>
      )}
      {showLeftSurfaceChromeBack ? (
        <div
          data-tauri-drag-region
          className="absolute top-0 left-0 z-50 h-12 w-[200px]"
        >
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-[9px] pl-[76px]"
          >
            <SidebarTimelineChromeButton
              ariaLabel="Go back"
              onClick={runEscapeShortcut}
            >
              <ArrowLeftIcon size={14} />
            </SidebarTimelineChromeButton>
          </div>
        </div>
      ) : null}
      <div className="flex min-h-0 min-w-0 flex-1 gap-1">
        <ClassicMainSidebar />
        <div
          className="min-h-0 min-w-0 flex-1 overflow-auto"
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
      </div>
    </div>
  );
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
  canGoBack,
  canGoNext,
  onBack,
  onForward,
}: {
  canGoBack: boolean;
  canGoNext: boolean;
  onBack: () => void;
  onForward: () => void;
}) {
  return (
    <div className="flex items-center gap-1">
      <SidebarTimelineChromeButton
        ariaLabel="Go back"
        disabled={!canGoBack}
        onClick={onBack}
      >
        <ArrowLeftIcon size={14} />
      </SidebarTimelineChromeButton>
      <SidebarTimelineChromeButton
        ariaLabel="Go forward"
        disabled={!canGoNext}
        onClick={onForward}
      >
        <ArrowRightIcon size={14} />
      </SidebarTimelineChromeButton>
    </div>
  );
}

function SidebarTimelineChromeButton({
  ariaLabel,
  children,
  disabled = false,
  onClick,
}: {
  ariaLabel: string;
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
        "flex size-7 items-center justify-center rounded-full",
        "text-neutral-700 transition-colors hover:bg-neutral-100 hover:text-neutral-900",
        "focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
        "disabled:text-neutral-300 disabled:hover:bg-transparent disabled:hover:text-neutral-300",
      ])}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
