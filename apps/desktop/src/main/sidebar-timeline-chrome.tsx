import {
  MagnifyingGlass,
  NotePencil,
  Sidebar,
  SidebarSimple,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { memo, type ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import type { SidebarNoteFilter } from "~/sidebar/note-filter";
import { SidebarNoteFilterMenu } from "~/sidebar/note-filter-menu";
import { useSidebarUpcomingMeetingStatus } from "~/sidebar/timeline/upcoming-meeting";

export const SidebarTimelineChromeWithUpcomingMeeting = memo(
  function SidebarTimelineChromeWithUpcomingMeeting({
    currentSessionId,
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
    noteFilter: SidebarNoteFilter;
    onNewNote: () => void;
    onNoteFilterChange: (filter: SidebarNoteFilter) => void;
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
  hasUpcomingMeeting,
  noteFilter,
  onNewNote,
  onNoteFilterChange,
  onSearch,
  onToggleSidebar,
  sidebarExpanded,
  showSidebarToggle,
}: {
  hasUpcomingMeeting: boolean;
  noteFilter: SidebarNoteFilter;
  onNewNote: () => void;
  onNoteFilterChange: (filter: SidebarNoteFilter) => void;
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
    <div data-tauri-drag-region {...stylex.props(styles.root)}>
      <div data-tauri-drag-region {...stylex.props(styles.controls)}>
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
            {...stylex.props(styles.placeholder)}
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
      {...stylex.props(styles.button)}
      onClick={onClick}
    >
      {children}
      {badge ? (
        <span
          aria-hidden="true"
          data-testid="collapsed-sidebar-upcoming-meeting-badge"
          {...stylex.props(styles.badge)}
        />
      ) : null}
    </button>
  );
}

const styles = stylex.create({
  badge: {
    backgroundColor: "rgb(239 68 68)",
    borderRadius: radii.full,
    boxShadow: `0 0 0 2px ${colors.background}`,
    height: "0.375rem",
    pointerEvents: "none",
    position: "absolute",
    right: "0.25rem",
    top: "0.25rem",
    width: "0.375rem",
  },
  button: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
      ":disabled:hover": "transparent",
    },
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
      ":disabled": `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
      ":disabled:hover": `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    },
    display: "flex",
    height: "1.75rem",
    justifyContent: "center",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    pointerEvents: "auto",
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
  },
  controls: {
    alignItems: "center",
    display: "flex",
    gap: 0,
  },
  placeholder: {
    flexShrink: 0,
    height: "1.75rem",
    width: "1.75rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    width: "100%",
  },
});
