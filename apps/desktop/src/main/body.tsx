import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
} from "lucide-react";
import { type MouseEvent, type PointerEvent, useCallback, useRef } from "react";

import { cn } from "@hypr/utils";

import { resolveMainSurfaceChrome } from "./main-surface-chrome";
import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabContent } from "./tab-content";
import { TopMeetingTimeline } from "./top-meeting-timeline";
import {
  type DesktopUpdateControl,
  SidebarTimelineUpdateButton,
  TimelineUpdateBanner,
  useDesktopUpdateControl,
} from "./update-banner";
import { useClassicMainTabsShortcuts } from "./useTabsShortcuts";

import { useShell } from "~/contexts/shell";
import { GlobalLiveTranscriptAccessory } from "~/session/components/bottom-accessory/global-live";
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
  const isChangelog = currentTab?.type === "changelog";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimelineChrome =
    sidebarTimelineEnabled && !hasCustomSidebar && !isOnboarding;
  const showSidebarTimeline = showSidebarTimelineChrome && leftsidebar.expanded;
  const showTopTimeline =
    leftsidebar.expanded &&
    !showSidebarTimeline &&
    !hasCustomSidebar &&
    !isChangelog &&
    !isOnboarding;
  const showLeftSurfaceChromeBack = hasLeftSurfaceCustomSidebar;
  const enableMainAreaTopDrag =
    showSidebarTimelineChrome || hasLeftSurfaceCustomSidebar;
  const mainSurfaceChrome = resolveMainSurfaceChrome({
    hasLeftSurfaceCustomSidebar,
    isChangelog,
    leftSidebarExpanded: leftsidebar.expanded,
    showSidebarTimeline,
    showSidebarTimelineChrome,
    showTopTimeline,
  });
  const mainAreaTopDrag = useMainAreaTopWindowDrag(enableMainAreaTopDrag);
  const update = useDesktopUpdateControl();

  return (
    <div className="relative flex h-full min-w-0 flex-1 flex-col">
      {isOnboarding ? null : showSidebarTimelineChrome ? (
        <div
          data-tauri-drag-region
          className={cn([
            "absolute top-0 z-40 h-12 w-[200px]",
            leftsidebar.expanded ? "left-0" : "left-1",
          ])}
        >
          <div
            data-tauri-drag-region
            className="flex h-full min-w-0 items-start pt-[9px] pr-3 pl-[76px]"
          >
            <SidebarTimelineChrome
              sidebarExpanded={leftsidebar.expanded}
              canGoBack={canGoBack}
              canGoNext={canGoNext}
              onBack={goBack}
              onForward={goNext}
              onToggleSidebar={leftsidebar.toggleExpanded}
              update={update}
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
            <LeftSurfaceChromeButton
              ariaLabel="Go back"
              onClick={runEscapeShortcut}
            >
              <ArrowLeftIcon size={14} />
            </LeftSurfaceChromeButton>
          </div>
        </div>
      ) : null}
      {showTopTimeline ? <TimelineUpdateBanner update={update} /> : null}
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
          <GlobalLiveTranscriptAccessory
            currentTab={currentTab}
            surfaceChrome={mainSurfaceChrome}
          >
            {currentTab ? (
              <ClassicMainTabContent
                key={uniqueIdfromTab(currentTab)}
                tab={currentTab as Tab}
              />
            ) : null}
          </GlobalLiveTranscriptAccessory>
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
  onToggleSidebar,
  sidebarExpanded,
  update,
}: {
  canGoBack: boolean;
  canGoNext: boolean;
  onBack: () => void;
  onForward: () => void;
  onToggleSidebar: () => void;
  sidebarExpanded: boolean;
  update: DesktopUpdateControl;
}) {
  const updateVisible = Boolean(update.status && update.version);

  return (
    <div className="flex w-full items-center justify-between">
      <div className="flex items-center gap-0">
        <LeftSurfaceChromeButton
          ariaLabel={sidebarExpanded ? "Hide sidebar" : "Show sidebar"}
          badge={!sidebarExpanded && updateVisible}
          onClick={onToggleSidebar}
        >
          {sidebarExpanded ? (
            <PanelLeftCloseIcon size={14} />
          ) : (
            <PanelLeftOpenIcon size={14} />
          )}
        </LeftSurfaceChromeButton>
        <LeftSurfaceChromeButton
          ariaLabel="Go back"
          disabled={!canGoBack}
          onClick={onBack}
        >
          <ArrowLeftIcon size={14} />
        </LeftSurfaceChromeButton>
        <LeftSurfaceChromeButton
          ariaLabel="Go forward"
          disabled={!canGoNext}
          onClick={onForward}
        >
          <ArrowRightIcon size={14} />
        </LeftSurfaceChromeButton>
      </div>
      {sidebarExpanded ? <SidebarTimelineUpdateButton update={update} /> : null}
    </div>
  );
}

function LeftSurfaceChromeButton({
  ariaLabel,
  badge = false,
  children,
  disabled = false,
  onClick,
}: {
  ariaLabel: string;
  badge?: boolean;
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
        "relative flex size-7 items-center justify-center rounded-full",
        "text-neutral-700 transition-colors hover:bg-neutral-100 hover:text-neutral-900",
        "focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
        "disabled:text-neutral-300 disabled:hover:bg-transparent disabled:hover:text-neutral-300",
      ])}
      onClick={onClick}
    >
      {children}
      {badge ? (
        <span
          aria-hidden="true"
          data-testid="collapsed-sidebar-update-badge"
          className="pointer-events-none absolute top-1 right-1 size-1.5 rounded-full bg-red-500 ring-2 ring-stone-50"
        />
      ) : null}
    </button>
  );
}
