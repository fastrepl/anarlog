import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { format } from "date-fns";
import { useCallback, useMemo } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";

import { toTz, useTimezone } from "~/calendar/hooks";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import { EventDisplay } from "~/session/components/outer-header/metadata";
import { getOrCreateSessionForEventId } from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import type { TimelineEventRow } from "~/sidebar/timeline/utils";
import { useTabs } from "~/store/zustand/tabs";

export function EventChip({
  eventId,
  event,
}: {
  eventId: string;
  event: TimelineEventRow | undefined;
}) {
  const tz = useTimezone();
  const { ignoreEvent, ignoreSeries } = useIgnoredEvents();
  const title = event?.title ?? undefined;
  const trackingId = event?.tracking_id_event ?? undefined;
  const recurrenceSeriesId = event?.recurrence_series_id ?? undefined;
  const isAllDay = !!event?.is_all_day;
  const color = event?.calendar_color || "#888";

  const startedAt = event?.started_at
    ? format(toTz(event.started_at, tz), "h:mm a")
    : null;

  const handleIgnore = useCallback(() => {
    if (!trackingId) {
      return;
    }

    ignoreEvent(trackingId);
  }, [trackingId, ignoreEvent]);

  const handleIgnoreSeries = useCallback(() => {
    if (!recurrenceSeriesId) {
      return;
    }

    ignoreSeries(recurrenceSeriesId);
  }, [recurrenceSeriesId, ignoreSeries]);

  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const menu: MenuItemDef[] = [
      {
        id: "ignore",
        text: recurrenceSeriesId ? "Delete This Event" : "Delete Event",
        action: handleIgnore,
      },
    ];

    if (recurrenceSeriesId) {
      menu.push({
        id: "ignore-series",
        text: "Delete All Recurring Events",
        action: handleIgnoreSeries,
      });
    }

    return menu;
  }, [recurrenceSeriesId, handleIgnore, handleIgnoreSeries]);
  const showContextMenu = useNativeContextMenu(contextMenu);

  if (!event || !title) {
    return null;
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        {isAllDay ? (
          <button
            {...stylex.props(styles.allDayChip, styles.color(color))}
            onContextMenu={showContextMenu}
          >
            {title}
          </button>
        ) : (
          <button
            {...stylex.props(styles.timedChip)}
            onContextMenu={showContextMenu}
          >
            <div {...stylex.props(styles.eventMarker, styles.color(color))} />
            <span {...stylex.props(styles.truncate)}>{title}</span>
            {startedAt && (
              <span {...stylex.props(styles.time)}>{startedAt}</span>
            )}
          </button>
        )}
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        sx={styles.popover}
        onClick={(e) => e.stopPropagation()}
      >
        <AppFloatingPanel>
          <EventPopoverContent eventId={eventId} event={event} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function EventPopoverContent({
  eventId,
  event,
}: {
  eventId: string;
  event: TimelineEventRow;
}) {
  const openCurrent = useTabs((state) => state.openCurrent);

  const openNote = useMutation({
    mutationFn: () =>
      getOrCreateSessionForEventId(eventId, event.title || "Untitled"),
    onSuccess: (sessionId) => {
      openCurrent({ type: "sessions", id: sessionId });
    },
    onError: (error) => {
      console.error("[calendar] failed to open event note", error);
    },
  });

  return (
    <div {...stylex.props(styles.popoverContent)}>
      <EventDisplay
        event={{
          title: event.title ?? undefined,
          startedAt: event.started_at ?? undefined,
          endedAt: event.ended_at ?? undefined,
          location: event.location ?? undefined,
          meetingLink: event.meeting_link ?? undefined,
          description: event.description ?? undefined,
          calendarId: event.calendar_id ?? undefined,
        }}
      />
      <Button
        size="sm"
        sx={styles.openButton}
        disabled={openNote.isPending}
        onClick={() => openNote.mutate()}
      >
        {openNote.isPending ? (
          <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
        ) : null}
        Open note
      </Button>
    </div>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  allDayChip: {
    borderRadius: "0.25rem",
    color: colors.primaryForeground,
    cursor: "pointer",
    fontSize: "0.75rem",
    lineHeight: 1.25,
    opacity: {
      default: 1,
      ":hover": 0.8,
    },
    overflow: "hidden",
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
    textAlign: "left",
    textOverflow: "ellipsis",
    userSelect: "none",
    whiteSpace: "nowrap",
    width: "100%",
  },
  color: (color: string) => ({
    backgroundColor: color,
  }),
  eventMarker: {
    alignSelf: "stretch",
    borderRadius: radii.full,
    flexShrink: 0,
    width: "2.5px",
  },
  openButton: {
    minHeight: "2rem",
    width: "100%",
  },
  popover: {
    display: "flex",
    flexDirection: "column",
    maxHeight: "80vh",
    width: "280px",
  },
  popoverContent: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    padding: "1rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.875rem",
    width: "0.875rem",
  },
  time: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontFamily: fonts.mono,
    marginLeft: "auto",
  },
  timedChip: {
    alignItems: "center",
    borderRadius: "0.25rem",
    cursor: "pointer",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: 1.25,
    opacity: {
      default: 1,
      ":hover": 0.8,
    },
    paddingLeft: "0.125rem",
    textAlign: "left",
    userSelect: "none",
    width: "100%",
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
