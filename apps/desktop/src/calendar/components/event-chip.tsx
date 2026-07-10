import { format } from "date-fns";
import { useCallback, useMemo } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

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
            className={cn([
              "text-primary-foreground w-full truncate rounded px-1.5 py-0.5 text-left text-xs leading-tight",
              "cursor-pointer select-none hover:opacity-80",
            ])}
            style={{ backgroundColor: color }}
            onContextMenu={showContextMenu}
          >
            {title}
          </button>
        ) : (
          <button
            className={cn([
              "flex w-full items-center gap-1 rounded pl-0.5 text-left text-xs leading-tight",
              "cursor-pointer select-none hover:opacity-80",
            ])}
            onContextMenu={showContextMenu}
          >
            <div
              className="w-[2.5px] shrink-0 self-stretch rounded-full"
              style={{ backgroundColor: color }}
            />
            <span className="truncate">{title}</span>
            {startedAt && (
              <span className="text-muted-foreground ml-auto shrink-0 font-mono">
                {startedAt}
              </span>
            )}
          </button>
        )}
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        className="flex max-h-[80vh] w-[280px] flex-col"
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

  const handleOpen = useCallback(() => {
    void getOrCreateSessionForEventId(eventId, event.title || "Untitled")
      .then((sessionId) => {
        openCurrent({ type: "sessions", id: sessionId });
      })
      .catch((error) => {
        console.error("[calendar] failed to open event note", error);
      });
  }, [eventId, event.title, openCurrent]);

  return (
    <div className="flex flex-col gap-3 p-4">
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
        className="bg-primary text-primary-foreground hover:bg-primary/90 min-h-8 w-full"
        onClick={handleOpen}
      >
        Open note
      </Button>
    </div>
  );
}
