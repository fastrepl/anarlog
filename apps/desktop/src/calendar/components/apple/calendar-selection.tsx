import { RefreshCwIcon } from "lucide-react";
import { useCallback, useEffect, useMemo } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useSync } from "../context";
import { SyncIndicator } from "./status";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import * as main from "~/store/tinybase/store/main";

export function AppleCalendarSelection({
  calendarClassName,
  leftAction,
}: { calendarClassName?: string; leftAction?: React.ReactNode } = {}) {
  const { groups, handleToggle, handleRefresh, isLoading } =
    useAppleCalendarSelection();

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <div>{leftAction}</div>

        <div className="flex items-center gap-2">
          <SyncIndicator />

          <Button
            variant="ghost"
            size="icon"
            onClick={handleRefresh}
            className="size-6"
            disabled={isLoading}
          >
            <RefreshCwIcon
              className={cn(["size-3.5", isLoading && "animate-spin"])}
            />
          </Button>
        </div>
      </div>

      <CalendarSelection
        groups={groups}
        onToggle={handleToggle}
        className={calendarClassName}
      />
    </div>
  );
}

export function useAppleCalendarSelection() {
  const { status, scheduleSync, scheduleDebouncedSync, cancelDebouncedSync } =
    useSync();

  const store = main.UI.useStore(main.STORE_ID);
  const calendars = main.UI.useTable("calendars", main.STORE_ID);

  useEffect(() => {
    scheduleSync();
  }, [scheduleSync]);

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

    return Array.from(grouped.entries()).map(([sourceName, calendars]) => ({
      sourceName,
      calendars,
    }));
  }, [calendars]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      store?.setPartialRow("calendars", calendar.id, { enabled });
      scheduleDebouncedSync();
    },
    [store, scheduleDebouncedSync],
  );

  const handleRefresh = useCallback(() => {
    cancelDebouncedSync();
    scheduleSync();
  }, [scheduleSync, cancelDebouncedSync]);

  return {
    groups,
    handleToggle,
    handleRefresh,
    isLoading: status === "syncing",
  };
}
