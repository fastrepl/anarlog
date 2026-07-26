import { format } from "date-fns";
import { useEffect, useRef, useState } from "react";

import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import { EventChip } from "./event-chip";
import { SessionChip } from "./session-chip";

import type { CalendarData } from "~/calendar/hooks";
import { useNow } from "~/calendar/hooks";

function useVisibleItemCount(
  ref: React.RefObject<HTMLDivElement | null>,
  totalItems: number,
) {
  const [maxVisible, setMaxVisible] = useState(totalItems);

  useEffect(() => {
    const el = ref.current;
    if (!el || totalItems === 0) return;

    const compute = () => {
      const available = el.clientHeight;
      const children = Array.from(el.children) as HTMLElement[];
      if (children.length === 0 || available <= 0) return;

      const chipH = children[0].offsetHeight;
      if (chipH === 0) return;

      const gap = parseFloat(getComputedStyle(el).rowGap) || 0;

      const allH = totalItems * chipH + Math.max(0, totalItems - 1) * gap;
      if (allH <= available) {
        setMaxVisible((prev) => (prev === totalItems ? prev : totalItems));
        return;
      }

      const overflowH = chipH;
      let count = 0;
      let used = 0;

      while (count < totalItems) {
        const next = chipH + (count > 0 ? gap : 0);
        const remaining = totalItems - count - 1;
        const moreSpace = remaining > 0 ? overflowH + gap : 0;
        if (used + next + moreSpace > available) break;
        used += next;
        count++;
      }

      const result = Math.max(1, count);
      setMaxVisible((prev) => (prev === result ? prev : result));
    };

    compute();
    const observer = new ResizeObserver(compute);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref, totalItems]);

  return maxVisible;
}

export function DayCell({
  day,
  isCurrentMonth,
  calendarData,
}: {
  day: Date;
  isCurrentMonth: boolean;
  calendarData: CalendarData;
}) {
  const dateKey = format(day, "yyyy-MM-dd");
  const eventIds = calendarData.eventIdsByDate[dateKey] ?? [];
  const sessionIds = calendarData.sessionIdsByDate[dateKey] ?? [];

  const now = useNow();
  const itemsRef = useRef<HTMLDivElement>(null);
  const totalItems = eventIds.length + sessionIds.length;
  const maxVisible = useVisibleItemCount(itemsRef, totalItems);
  const today = format(day, "yyyy-MM-dd") === format(now, "yyyy-MM-dd");

  const visibleEvents = eventIds.slice(0, maxVisible);
  const remainingSlots = Math.max(0, maxVisible - visibleEvents.length);
  const visibleSessions = sessionIds.slice(0, remainingSlots);
  const shownCount = visibleEvents.length + visibleSessions.length;
  const overflow = totalItems - shownCount;

  return (
    <div
      className={cn([
        "border-r-border border-b-border border-r border-b",
        "flex min-w-0 flex-col p-1.5 select-none",
        (day.getDay() === 0 || day.getDay() === 6) && "bg-muted",
      ])}
    >
      <div className="flex shrink-0 justify-end">
        <div
          className={cn([
            "mb-1 flex h-7 w-7 items-center justify-center rounded-full text-sm font-medium",
            today && "bg-primary text-primary-foreground",
            !today && !isCurrentMonth && "text-muted-foreground/70",
            !today &&
              isCurrentMonth &&
              (day.getDay() === 0 || day.getDay() === 6) &&
              "text-muted-foreground",
            !today &&
              isCurrentMonth &&
              day.getDay() !== 0 &&
              day.getDay() !== 6 &&
              "text-foreground",
          ])}
        >
          {format(day, "d")}
        </div>
      </div>
      <div
        ref={itemsRef}
        className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-hidden"
      >
        {visibleEvents.map((eventId) => (
          <EventChip
            key={eventId}
            eventId={eventId}
            event={calendarData.eventsById[eventId]}
          />
        ))}
        {visibleSessions.map((sessionId) => (
          <SessionChip
            key={sessionId}
            sessionId={sessionId}
            session={calendarData.sessionsById[sessionId]}
          />
        ))}
        {overflow > 0 && (
          <Popover>
            <PopoverTrigger asChild>
              <button className="text-muted-foreground hover:text-muted-foreground shrink-0 cursor-pointer pl-1 text-left text-xs">
                +{overflow} more
              </button>
            </PopoverTrigger>
            <PopoverContent
              variant="app"
              align="start"
              className="max-h-[300px] w-[220px] overflow-y-auto"
              onClick={(e) => e.stopPropagation()}
            >
              <AppFloatingPanel className="p-2">
                <div className="text-foreground mb-2 text-sm font-medium">
                  {format(day, "MMM d, yyyy")}
                </div>
                <div className="flex flex-col gap-0.5">
                  {eventIds.map((eventId) => (
                    <EventChip
                      key={eventId}
                      eventId={eventId}
                      event={calendarData.eventsById[eventId]}
                    />
                  ))}
                  {sessionIds.map((sessionId) => (
                    <SessionChip
                      key={sessionId}
                      sessionId={sessionId}
                      session={calendarData.sessionsById[sessionId]}
                    />
                  ))}
                </div>
              </AppFloatingPanel>
            </PopoverContent>
          </Popover>
        )}
      </div>
    </div>
  );
}
