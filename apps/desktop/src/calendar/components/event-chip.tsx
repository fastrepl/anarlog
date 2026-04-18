import { format } from "date-fns";
import { useCallback } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import {
  toTz,
  useCalendar,
  useEvent,
  useTimelineEvent,
  useTimezone,
} from "~/calendar/hooks";
import { EventDisplay } from "~/session/components/outer-header/metadata";
import { useGetOrCreateSessionForEvent } from "~/session/hooks/runtime";
import { useTabs } from "~/store/zustand/tabs";

function useCalendarColor(calendarId: string | null): string | null {
  const calendar = useCalendar(calendarId);
  return calendar?.color || null;
}

export function EventChip({ eventId }: { eventId: string }) {
  const tz = useTimezone();
  const event = useTimelineEvent(eventId);
  const calendarColor = useCalendarColor(event?.calendarId ?? null);

  if (!event || !event.title) {
    return null;
  }

  const isAllDay = event.isAllDay;
  const color = calendarColor ?? "#888";

  const startedAt = event.startedAt
    ? format(toTz(event.startedAt, tz), "h:mm a")
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
            {event.title}
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
            <span className="truncate">{event.title}</span>
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
  const openNew = useTabs((state) => state.openNew);
  const eventRow = useTimelineEvent(eventId);
  const getOrCreateSession = useGetOrCreateSessionForEvent();

  const handleOpen = useCallback(() => {
    const title = eventRow?.title || "Untitled";
    const sessionId = getOrCreateSession(eventId, title);
    if (sessionId) {
      openNew({ type: "sessions", id: sessionId });
    }
  }, [eventId, eventRow?.title, openNew, getOrCreateSession]);

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
