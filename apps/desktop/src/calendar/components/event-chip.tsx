import { format } from "date-fns";
import { useCallback } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import { toTz, useTimezone } from "~/calendar/hooks";
import { EventDisplay } from "~/session/components/outer-header/metadata";
import { useEvent } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";
import { useTabs } from "~/store/zustand/tabs";

function useCalendarColor(calendarId: string | null): string | null {
  const calendar = main.UI.useRow("calendars", calendarId ?? "", main.STORE_ID);
  if (!calendarId) return null;
  return calendar?.color ? String(calendar.color) : null;
}

export function EventChip({ eventId }: { eventId: string }) {
  const tz = useTimezone();
  const event = main.UI.useResultRow(
    main.QUERIES.timelineEvents,
    eventId,
    main.STORE_ID,
  );
  const calendarColor = useCalendarColor(
    (event?.calendar_id as string) ?? null,
  );

  if (!event || !event.title) {
    return null;
  }

  const isAllDay = !!event.is_all_day;
  const color = calendarColor ?? "#888";

  const startedAt = event.started_at
    ? format(toTz(event.started_at as string, tz), "h:mm a")
    : null;

  return (
    <Popover>
      <PopoverTrigger asChild>
        {isAllDay ? (
          <button
            className={cn([
              "w-full truncate rounded px-1.5 py-0.5 text-left text-xs leading-tight text-white",
              "cursor-pointer hover:opacity-80",
            ])}
            style={{ backgroundColor: color }}
          >
            {event.title as string}
          </button>
        ) : (
          <button
            className={cn([
              "flex w-full items-center gap-1 rounded pl-0.5 text-left text-xs leading-tight",
              "cursor-pointer hover:opacity-80",
            ])}
          >
            <div
              className="w-[2.5px] shrink-0 self-stretch rounded-full"
              style={{ backgroundColor: color }}
            />
            <span className="truncate">{event.title as string}</span>
            {startedAt && (
              <span className="ml-auto shrink-0 font-mono text-neutral-400">
                {startedAt}
              </span>
            )}
          </button>
        )}
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="flex max-h-[80vh] w-[280px] flex-col rounded-lg p-0 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <EventPopoverContent eventId={eventId} />
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
