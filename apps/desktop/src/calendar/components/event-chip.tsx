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

import { toTz, useCalendar, useTimezone } from "~/calendar/hooks";
import { EventDisplay } from "~/session/components/outer-header/metadata";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useEvent, useIgnoredEvents } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";
import { useTabs } from "~/store/zustand/tabs";

function useCalendarColor(calendarId: string | null): string | null {
  const calendar = useCalendar(calendarId);
  return calendar?.color || null;
}

export function EventChip({ eventId }: { eventId: string }) {
  const tz = useTimezone();
  const store = main.UI.useStore(main.STORE_ID);
  const openNew = useTabs((state) => state.openNew);
  const { ignoreEvent, ignoreSeries } = useIgnoredEvents();
  const event = main.UI.useResultRow(
    main.QUERIES.timelineEvents,
    eventId,
    main.STORE_ID,
  );
  const calendarColor = useCalendarColor(
    (event?.calendar_id as string) ?? null,
  );
  const title = event?.title as string | undefined;
  const trackingId = event?.tracking_id_event as string | undefined;
  const recurrenceSeriesId = event?.recurrence_series_id as string | undefined;
  const isAllDay = !!event?.is_all_day;
  const color = calendarColor ?? "#888";

  const startedAt = event?.started_at
    ? format(toTz(event.started_at as string, tz), "h:mm a")
    : null;

  const handleOpenNewTab = useCallback(() => {
    if (!store || !title) {
      return;
    }

    const sessionId = getOrCreateSessionForEventId(store, eventId, title);
    openNew({ type: "sessions", id: sessionId });
  }, [store, eventId, title, openNew]);

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
        id: "open-new-tab",
        text: "Open in New Tab",
        action: handleOpenNewTab,
      },
      { separator: true },
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
  }, [recurrenceSeriesId, handleOpenNewTab, handleIgnore, handleIgnoreSeries]);
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
              "w-full truncate rounded px-1.5 py-0.5 text-left text-xs leading-tight text-white",
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
              <span className="ml-auto shrink-0 font-mono text-neutral-400">
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
          <EventPopoverContent eventId={eventId} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function EventPopoverContent({ eventId }: { eventId: string }) {
  const event = useEvent(eventId);
  const store = main.UI.useStore(main.STORE_ID);
  const openNew = useTabs((state) => state.openNew);

  const eventRow = main.UI.useResultRow(
    main.QUERIES.timelineEvents,
    eventId,
    main.STORE_ID,
  );

  const handleOpen = useCallback(() => {
    if (!store) return;
    const title = (eventRow?.title as string) || "Untitled";
    const sessionId = getOrCreateSessionForEventId(store, eventId, title);
    openNew({ type: "sessions", id: sessionId });
  }, [store, eventId, eventRow?.title, openNew]);

  if (!event) {
    return null;
  }

  return (
    <div className="flex flex-col gap-3 p-4">
      <EventDisplay event={event} />
      <Button
        size="sm"
        className="min-h-8 w-full bg-stone-800 text-white hover:bg-stone-700"
        onClick={handleOpen}
      >
        Open note
      </Button>
    </div>
  );
}
