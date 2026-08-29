import {
  MagnifyingGlass,
  NotePencil,
  Sidebar,
  SidebarSimple,
} from "@phosphor-icons/react";
import { memo, type ReactNode } from "react";

import { cn } from "@anlg/utils";

import type { SidebarNoteFilter } from "~/sidebar/note-filter";
import { SidebarNoteFilterMenu } from "~/sidebar/note-filter-menu";
import { useSidebarUpcomingMeetingStatus } from "~/sidebar/timeline/upcoming-meeting";

export const SidebarTimelineChromeWithUpcomingMeeting = memo(
  function SidebarTimelineChromeWithUpcomingMeeting({
    currentSessionId,
    folderFilter = null,
    noteFilter,
    onNewNote,
    onNoteFilterChange,
    onSearch,
    onToggleSidebar,
    sidebarExpanded,
    showSidebarToggle = true,
    showIgnoredTimelineEvents,
  }: {
    currentSessionId?: string;
    folderFilter?: string | null;
    noteFilter: SidebarNoteFilter;
    onNewNote: () => void;
    onNoteFilterChange: (
      filter: SidebarNoteFilter,
      folderFilter?: string | null,
    ) => void;
    onSearch: () => void;
    onToggleSidebar: () => void;
    sidebarExpanded: boolean;
    showSidebarToggle?: boolean;
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
        folderFilter={folderFilter}
        hasUpcomingMeeting={hasUpcomingMeeting}
        noteFilter={noteFilter}
        onNewNote={onNewNote}
        onNoteFilterChange={onNoteFilterChange}
        onSearch={onSearch}
        onToggleSidebar={onToggleSidebar}
        sidebarExpanded={sidebarExpanded}
        showSidebarToggle={showSidebarToggle}
      />
    );
  },
);

function SidebarTimelineChrome({
  folderFilter,
  hasUpcomingMeeting,
  noteFilter,
  onNewNote,
  onNoteFilterChange,
  onSearch,
  onToggleSidebar,
  sidebarExpanded,
  showSidebarToggle,
}: {
  folderFilter: string | null;
  hasUpcomingMeeting: boolean;
  noteFilter: SidebarNoteFilter;
  onNewNote: () => void;
  onNoteFilterChange: (
    filter: SidebarNoteFilter,
    folderFilter?: string | null,
  ) => void;
  onSearch: () => void;
  onToggleSidebar: () => void;
  sidebarExpanded: boolean;
  showSidebarToggle: boolean;
}) {
  const collapsedBadge = !sidebarExpanded
    ? hasUpcomingMeeting
      ? "upcomingMeeting"
      : null
    : null;

  return (
    <div data-tauri-drag-region className="flex w-full items-center">
      <div data-tauri-drag-region className="flex items-center gap-0">
        {showSidebarToggle ? (
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
        ) : (
          <span
            aria-hidden="true"
            data-tauri-drag-region
            className="size-7 shrink-0"
          />
        )}
        {sidebarExpanded ? (
          <>
            <LeftSurfaceChromeButton ariaLabel="Search" onClick={onSearch}>
              <MagnifyingGlass size={15} />
            </LeftSurfaceChromeButton>
            <LeftSurfaceChromeButton ariaLabel="New note" onClick={onNewNote}>
              <NotePencil size={15} />
            </LeftSurfaceChromeButton>
            <SidebarNoteFilterMenu
              folderFilter={folderFilter}
              value={noteFilter}
              onValueChange={onNoteFilterChange}
            />
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
