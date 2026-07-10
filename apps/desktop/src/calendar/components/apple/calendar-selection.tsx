import { useCallback, useMemo } from "react";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import { setCalendarEnabled, useCalendarRows } from "~/calendar/queries";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

const SUBSCRIBED_SOURCE_NAME = "Subscribed Calendars";

export function AppleCalendarSelection({
  calendarClassName,
  leftAction,
}: { calendarClassName?: string; leftAction?: React.ReactNode } = {}) {
  const { groups, handleRefresh, handleToggle, scheduleSync } =
    useAppleCalendarSelection();

  useMountEffect(() => {
    if (groups.length === 0) {
      scheduleSync();
    }
  });

  return (
    <div className="flex flex-col gap-2">
      {leftAction && groups.length === 0 ? <div>{leftAction}</div> : null}

      <CalendarSelection
        groups={groups}
        onToggle={handleToggle}
        onRefresh={handleRefresh}
        className={calendarClassName}
      />
    </div>
  );
}

export function useAppleCalendarSelection() {
  const { cancelDebouncedSync, status, scheduleDebouncedSync, scheduleSync } =
    useSync();

  const calendars = useCalendarRows("apple");

  const groups = useMemo((): CalendarGroup[] => {
    const grouped = new Map<string, CalendarItem[]>();
    for (const cal of calendars) {
      const source = cal.source || "Apple Calendar";
      if (!grouped.has(source)) grouped.set(source, []);
      grouped.get(source)!.push({
        id: cal.id,
        title: cal.name || "Untitled",
        color: cal.color ?? "#888",
        enabled: cal.enabled ?? false,
      });
    }

    return Array.from(grouped.entries())
      .map(([sourceName, calendars]) => ({
        sourceName,
        calendars,
      }))
      .sort((a, b) => {
        if (a.sourceName === SUBSCRIBED_SOURCE_NAME) return 1;
        if (b.sourceName === SUBSCRIBED_SOURCE_NAME) return -1;
        return 0;
      });
  }, [calendars]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      void setCalendarEnabled(calendar.id, enabled)
        .then(scheduleDebouncedSync)
        .catch((error) => {
          console.error("[calendar] failed to update calendar", error);
        });
    },
    [scheduleDebouncedSync],
  );

  const handleRefresh = useCallback(() => {
    cancelDebouncedSync();
    scheduleSync();
  }, [cancelDebouncedSync, scheduleSync]);

  return {
    groups,
    handleRefresh,
    handleToggle,
    isLoading: status === "syncing",
    scheduleSync,
  };
}
