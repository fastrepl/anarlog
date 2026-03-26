import { useCallback, useMemo } from "react";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import * as main from "~/store/tinybase/store/main";

const SUBSCRIBED_SOURCE_NAME = "Subscribed Calendars";

export function AppleCalendarSelection({
  calendarClassName,
  leftAction,
}: { calendarClassName?: string; leftAction?: React.ReactNode } = {}) {
  const { groups, handleToggle, scheduleSync } = useAppleCalendarSelection();

  useMountEffect(() => {
    if (groups.length === 0) {
      scheduleSync();
    }
  });

  return (
    <div className="flex flex-col gap-2">
      {leftAction ? <div>{leftAction}</div> : null}

      <CalendarSelection
        groups={groups}
        onToggle={handleToggle}
        className={calendarClassName}
      />
    </div>
  );
}

export function useAppleCalendarSelection() {
  const { status, scheduleDebouncedSync, scheduleSync } = useSync();

  const store = main.UI.useStore(main.STORE_ID);
  const calendars = main.UI.useTable("calendars", main.STORE_ID);

  const groups = useMemo((): CalendarGroup[] => {
    const appleCalendars = Object.entries(calendars).filter(
      ([_, cal]) => cal.provider === "apple",
    );

    const grouped = new Map<string, CalendarItem[]>();
    for (const [id, cal] of appleCalendars) {
      const source = cal.source || "Apple Calendar";
      if (!grouped.has(source)) grouped.set(source, []);
      grouped.get(source)!.push({
        id,
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
      store?.setPartialRow("calendars", calendar.id, { enabled });
      scheduleDebouncedSync();
    },
    [store, scheduleDebouncedSync],
  );

  return {
    groups,
    handleToggle,
    isLoading: status === "syncing",
    scheduleSync,
  };
}
