import {
  MagnifyingGlass,
  NotePencil,
  Sidebar,
  SidebarSimple,
  Wrench,
} from "@phosphor-icons/react";
import { memo, type ReactNode } from "react";

import { cn } from "@anlg/utils";

import type { SidebarNoteFilter } from "~/sidebar/note-filter";
import { SidebarNoteFilterMenu } from "~/sidebar/note-filter-menu";
import { useSidebarUpcomingMeetingStatus } from "~/sidebar/timeline/upcoming-meeting";

export const SidebarTimelineChromeWithUpcomingMeeting = memo(
  function SidebarTimelineChromeWithUpcomingMeeting({
    currentSessionId,
    devtoolsPanelOpen,
    noteFilter,
    onNewNote,
    onNoteFilterChange,
    onOpenDevtools,
    onSearch,
    onToggleSidebar,
    sidebarExpanded,
    showDevtoolsPanelButton,
    showIgnoredTimelineEvents,
  }: {
    currentSessionId?: string;
    devtoolsPanelOpen: boolean;
    noteFilter: SidebarNoteFilter;
    onNewNote: () => void;
    onNoteFilterChange: (filter: SidebarNoteFilter) => void;
    onOpenDevtools: () => void;
    onSearch: () => void;
    onToggleSidebar: () => void;
    sidebarExpanded: boolean;
    showDevtoolsPanelButton: boolean;
    showIgnoredTimelineEvents: boolean;
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
        noteFilter={noteFilter}
        onNewNote={onNewNote}
        onNoteFilterChange={onNoteFilterChange}
        onOpenDevtools={onOpenDevtools}
        onSearch={onSearch}
        onToggleSidebar={onToggleSidebar}
        sidebarExpanded={sidebarExpanded}
        showDevtoolsPanelButton={showDevtoolsPanelButton}
      />
    );
  },
);

function SidebarTimelineChrome({
  devtoolsPanelOpen,
  hasUpcomingMeeting,
  noteFilter,
  onNewNote,
  onNoteFilterChange,
  onOpenDevtools,
  onSearch,
  onToggleSidebar,
  sidebarExpanded,
  showDevtoolsPanelButton,
}: {
  devtoolsPanelOpen: boolean;
  hasUpcomingMeeting: boolean;
  noteFilter: SidebarNoteFilter;
  onNewNote: () => void;
  onNoteFilterChange: (filter: SidebarNoteFilter) => void;
  onOpenDevtools: () => void;
  onSearch: () => void;
  onToggleSidebar: () => void;
  sidebarExpanded: boolean;
  showDevtoolsPanelButton: boolean;
}) {
  const collapsedBadge = !sidebarExpanded
    ? hasUpcomingMeeting
      ? "upcomingMeeting"
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
            <SidebarNoteFilterMenu
              value={noteFilter}
              onValueChange={onNoteFilterChange}
            />
            {showDevtoolsPanelButton && !devtoolsPanelOpen ? (
              <LeftSurfaceChromeButton
                ariaLabel="Show devtools panel"
                onClick={onOpenDevtools}
              >
                <Wrench size={15} />
              </LeftSurfaceChromeButton>
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
  badge?: "upcomingMeeting" | null;
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
          data-testid="collapsed-sidebar-upcoming-meeting-badge"
          className="ring-background pointer-events-none absolute top-1 right-1 size-1.5 rounded-full bg-red-500 ring-2"
        />
      ) : null}
    </button>
  );
}
