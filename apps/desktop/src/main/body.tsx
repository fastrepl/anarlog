import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeftIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  SearchIcon,
  SquarePenIcon,
} from "lucide-react";
import { type MouseEvent, type PointerEvent, useCallback, useRef } from "react";

import { cn } from "@hypr/utils";

import { resolveMainSurfaceChrome } from "./main-surface-chrome";
import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabContent } from "./tab-content";
import {
  type DesktopUpdateControl,
  SidebarTimelineUpdateButton,
  useDesktopUpdateControl,
} from "./update-banner";
import { useClassicMainTabsShortcuts } from "./useTabsShortcuts";

import { useShell } from "~/contexts/shell";
import { GlobalLiveTranscriptAccessory } from "~/session/components/bottom-accessory/global-live";
import { useOpenNoteDialog } from "~/shared/open-note-dialog";
import { useNewNote } from "~/shared/useNewNote";
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
  const { runEscapeShortcut } = useClassicMainTabsShortcuts();

  const isOnboarding = currentTab?.type === "onboarding";
  const isChangelog = currentTab?.type === "changelog";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const hasLeftSurfaceCustomSidebar =
    hasLeftSurfaceCustomSidebarTab(currentTab);
  const showSidebarTimelineChrome = !hasCustomSidebar && !isOnboarding;
  const showSidebarTimeline = showSidebarTimelineChrome && leftsidebar.expanded;
  const showLeftSurfaceChromeBack = hasLeftSurfaceCustomSidebar;
  const enableMainAreaTopDrag =
    showSidebarTimelineChrome || hasLeftSurfaceCustomSidebar;
  const mainSurfaceChrome = resolveMainSurfaceChrome({
    hasLeftSurfaceCustomSidebar,
    isChangelog,
    leftSidebarExpanded: leftsidebar.expanded,
    showSidebarTimeline,
    showSidebarTimelineChrome,
  });
  const mainAreaTopDrag = useMainAreaTopWindowDrag(enableMainAreaTopDrag);
  const update = useDesktopUpdateControl();
  const createNewNote = useNewNote();
  const openNoteDialog = useOpenNoteDialog();
  const handleOpenNoteDialog = useCallback(() => {
    openNoteDialog.open();
  }, [openNoteDialog]);

  return (
    <div className="relative flex h-full min-w-0 flex-1 flex-col">
      {isOnboarding ? null : showSidebarTimelineChrome ? (
        <div
          data-tauri-drag-region
          className={cn([
            "absolute top-0 z-40 h-12 w-[200px]",
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
              onNewNote={createNewNote}
              onSearch={handleOpenNoteDialog}
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
              <ArrowLeftIcon size={16} />
            </LeftSurfaceChromeButton>
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
  onNewNote,
  onSearch,
  onToggleSidebar,
  sidebarExpanded,
  update,
}: {
  onNewNote: () => void;
  onSearch: () => void;
  onToggleSidebar: () => void;
  sidebarExpanded: boolean;
  update: DesktopUpdateControl;
}) {
  const updateVisible = Boolean(update.status && update.version);
  const showDownloadButton = sidebarExpanded && update.status === "available";

  return (
    <div
      data-tauri-drag-region
      className="flex w-full items-center justify-between"
    >
      <div data-tauri-drag-region className="flex items-center gap-0">
        <LeftSurfaceChromeButton
          ariaLabel={sidebarExpanded ? "Hide sidebar" : "Show sidebar"}
          badge={!sidebarExpanded && updateVisible}
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
          </>
        ) : null}
      </div>
      {showDownloadButton ? (
        <SidebarTimelineUpdateButton update={update} />
      ) : null}
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
          data-testid="collapsed-sidebar-update-badge"
          className="ring-background pointer-events-none absolute top-1 right-1 size-1.5 rounded-full bg-red-500 ring-2"
        />
      ) : null}
    </button>
  );
}
