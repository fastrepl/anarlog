import * as stylex from "@stylexjs/stylex";
import { format } from "date-fns";
import { useEffect, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";

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
      {...stylex.props([
        styles.cell,
        (day.getDay() === 0 || day.getDay() === 6) && styles.weekend,
      ])}
    >
      <div {...stylex.props(styles.dayHeader)}>
        <div
          {...stylex.props([
            styles.dayNumber,
            today && styles.today,
            !today && !isCurrentMonth && styles.outsideMonth,
            !today &&
              isCurrentMonth &&
              (day.getDay() === 0 || day.getDay() === 6) &&
              styles.weekendDay,
            !today &&
              isCurrentMonth &&
              day.getDay() !== 0 &&
              day.getDay() !== 6 &&
              styles.currentMonthDay,
          ])}
        >
          {format(day, "d")}
        </div>
      </div>
      <div ref={itemsRef} {...stylex.props(styles.items)}>
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
              <button {...stylex.props(styles.moreButton)}>
                +{overflow} more
              </button>
            </PopoverTrigger>
            <PopoverContent
              variant="app"
              align="start"
              sx={styles.popover}
              onClick={(e) => e.stopPropagation()}
            >
              <AppFloatingPanel sx={styles.popoverPanel}>
                <div {...stylex.props(styles.popoverTitle)}>
                  {format(day, "MMM d, yyyy")}
                </div>
                <div {...stylex.props(styles.popoverItems)}>
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

const styles = stylex.create({
  cell: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderRightColor: colors.border,
    borderRightStyle: "solid",
    borderRightWidth: "1px",
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
    padding: "0.375rem",
    userSelect: "none",
  },
  currentMonthDay: {
    color: colors.foreground,
  },
  dayHeader: {
    display: "flex",
    flexShrink: 0,
    justifyContent: "flex-end",
  },
  dayNumber: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    height: "1.75rem",
    justifyContent: "center",
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
    width: "1.75rem",
  },
  items: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    gap: "0.125rem",
    minHeight: 0,
    overflow: "hidden",
  },
  moreButton: {
    color: colors.mutedForeground,
    cursor: "pointer",
    flexShrink: 0,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    paddingLeft: "0.25rem",
    textAlign: "left",
  },
  outsideMonth: {
    color: `color-mix(in srgb, ${colors.mutedForeground} 70%, transparent)`,
  },
  popover: {
    maxHeight: "300px",
    overflowY: "auto",
    width: "220px",
  },
  popoverItems: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  popoverPanel: {
    padding: "0.5rem",
  },
  popoverTitle: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    marginBottom: "0.5rem",
  },
  today: {
    backgroundColor: colors.primary,
    color: colors.primaryForeground,
  },
  weekend: {
    backgroundColor: colors.muted,
  },
  weekendDay: {
    color: colors.mutedForeground,
  },
});
