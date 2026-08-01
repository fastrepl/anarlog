import {
  MagnifyingGlass,
  NotePencil,
  Sidebar,
  SidebarSimple,
  Wrench,
} from "@phosphor-icons/react";
import { memo, type ReactNode } from "react";

import { cn } from "@anlg/utils";

import {
  type DesktopUpdateControl,
  SidebarTimelineUpdateButton,
} from "./update-banner";

import { useSidebarUpcomingMeetingStatus } from "~/sidebar/timeline/upcoming-meeting";

export const SidebarTimelineChromeWithUpcomingMeeting = memo(
  function SidebarTimelineChromeWithUpcomingMeeting({
    currentSessionId,
    devtoolsPanelOpen,
    onNewNote,
    onOpenDevtools,
    onSearch,
    onToggleSidebar,
    sidebarExpanded,
    showDevtoolsPanelButton,
    showIgnoredTimelineEvents,
    update,
  }: {
    currentSessionId?: string;
    devtoolsPanelOpen: boolean;
    onNewNote: () => void;
    onOpenDevtools: () => void;
    onSearch: () => void;
    onToggleSidebar: () => void;
    sidebarExpanded: boolean;
    showDevtoolsPanelButton: boolean;
    showIgnoredTimelineEvents: boolean;
    update: DesktopUpdateControl;
  }) {
    const upcomingMeetingStatus = useSidebarUpcomingMeetingStatus({
      showIgnored: showIgnoredTimelineEvents,
    });
    const hasUpcomingMeeting = upcomingMeetingStatus
      ? !currentSessionId ||
        upcomingMeetingStatus.itemKey !== `session-${currentSessionId}`
      : false;

    return (
      <SidebarTimelineChrome
        devtoolsPanelOpen={devtoolsPanelOpen}
        hasUpcomingMeeting={hasUpcomingMeeting}
        onNewNote={onNewNote}
        onOpenDevtools={onOpenDevtools}
        onSearch={onSearch}
        onToggleSidebar={onToggleSidebar}
        sidebarExpanded={sidebarExpanded}
        showDevtoolsPanelButton={showDevtoolsPanelButton}
        update={update}
      />
    );
  },
);

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
    <div data-tauri-drag-region className="flex w-full items-center">
      <div data-tauri-drag-region className="flex items-center gap-0">
        <LeftSurfaceChromeButton
          ariaLabel={sidebarExpanded ? "Hide sidebar" : "Show sidebar"}
          badge={collapsedBadge}
          onClick={onToggleSidebar}
        >
          {sidebarExpanded ? (
            <SidebarSimple size={16} />
          ) : (
            <Sidebar size={16} />
          )}
        </LeftSurfaceChromeButton>
        {sidebarExpanded ? (
          <>
            <LeftSurfaceChromeButton ariaLabel="Search" onClick={onSearch}>
              <MagnifyingGlass size={15} />
            </LeftSurfaceChromeButton>
            <LeftSurfaceChromeButton ariaLabel="New note" onClick={onNewNote}>
              <NotePencil size={15} />
            </LeftSurfaceChromeButton>
            {showDevtoolsPanelButton && !devtoolsPanelOpen ? (
              <LeftSurfaceChromeButton
                ariaLabel="Show devtools panel"
                onClick={onOpenDevtools}
              >
                <Wrench size={15} />
              </LeftSurfaceChromeButton>
            ) : null}
            {showUpdateButton ? (
              <SidebarTimelineUpdateButton update={update} />
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  );
}

export function LeftSurfaceChromeButton({
  ariaLabel,
  badge = null,
  children,
  disabled = false,
  onClick,
}: {
  ariaLabel: string;
  badge?: "update" | "upcomingMeeting" | null;
  children: ReactNode;
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
