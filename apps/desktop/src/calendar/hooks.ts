import { format } from "date-fns";
import { useEffect, useMemo, useState } from "react";

import { safeParseDate } from "@hypr/utils";
import { TZDate } from "@hypr/utils";

import { useConfigValue } from "~/shared/config";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";

export function useTimezone() {
  return useConfigValue("timezone") || undefined;
}

export function toTz(date: Date | string, tz?: string): Date {
  const d = typeof date === "string" ? new Date(date) : date;
  return tz ? new TZDate(d, tz) : d;
}

export function useNow() {
  const tz = useTimezone();
  const [now, setNow] = useState(() => toTz(new Date(), tz));

  useEffect(() => {
    const interval = setInterval(() => {
      setNow(toTz(new Date(), tz));
    }, 60000);
    return () => clearInterval(interval);
  }, [tz]);

  return now;
}

function getSystemWeekStart(): 0 | 1 {
  const locale = navigator.language || "en-US";
  try {
    const options = new Intl.Locale(locale);
    const info = (options as any).getWeekInfo?.() ?? (options as any).weekInfo;
    if (info?.firstDay === 1) return 1;
  } catch {}
  return 0;
}

export function useWeekStartsOn(): 0 | 1 {
  const value = useConfigValue("week_start");
  return useMemo(() => {
    if (value === "monday") return 1;
    if (value === "sunday") return 0;
    return getSystemWeekStart();
  }, [value]);
}

export type CalendarData = {
  eventIdsByDate: Record<string, string[]>;
  sessionIdsByDate: Record<string, string[]>;
};

export function useCalendarData(): CalendarData {
  const tz = useTimezone();

  const eventsTable = main.UI.useResultTable(
    main.QUERIES.timelineEvents,
    main.STORE_ID,
  );
  const sessionsTable = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );
  const { isIgnored } = useIgnoredEvents();

  return useMemo(() => {
    const eventIdsByDate: Record<string, string[]> = {};
    const sessionIdsByDate: Record<string, string[]> = {};

    if (eventsTable) {
      for (const [eventId, row] of Object.entries(eventsTable)) {
        if (!row.title) continue;
        const raw = safeParseDate(row.started_at);
        if (!raw) continue;
        if (isIgnored(row.tracking_id_event, row.recurrence_series_id))
          continue;
        const day = format(toTz(raw, tz), "yyyy-MM-dd");
        (eventIdsByDate[day] ??= []).push(eventId);
      }

      for (const ids of Object.values(eventIdsByDate)) {
        ids.sort((a, b) => {
          const aAllDay = eventsTable[a]?.is_all_day ? 0 : 1;
          const bAllDay = eventsTable[b]?.is_all_day ? 0 : 1;
          return aAllDay - bAllDay;
        });
      }
    }

    if (sessionsTable) {
      for (const [sessionId, row] of Object.entries(sessionsTable)) {
        if (row.event_json || !row.title) continue;
        const raw = safeParseDate(row.created_at);
        if (!raw) continue;
        const key = format(toTz(raw, tz), "yyyy-MM-dd");
        (sessionIdsByDate[key] ??= []).push(sessionId);
      }
    }

    return { eventIdsByDate, sessionIdsByDate };
  }, [eventsTable, sessionsTable, tz, isIgnored]);
}
