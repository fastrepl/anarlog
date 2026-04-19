import { format } from "date-fns";
import { useCallback, useEffect, useMemo, useState } from "react";

import { commands as calendarCommands } from "@hypr/plugin-calendar";
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

export type Calendar = {
  id: string;
  tracking_id_calendar: string;
  name: string;
  enabled: boolean;
  provider: string;
  source: string;
  color: string;
  connection_id: string;
  created_at: string;
};

export function useCalendar(id: string | null | undefined): Calendar | null {
  const row = main.UI.useRow("calendars", id ?? "", main.STORE_ID);
  return useMemo(() => {
    if (!id) return null;
    if (!row || Object.keys(row).length === 0) return null;
    return {
      id,
      tracking_id_calendar: row.tracking_id_calendar ?? "",
      name: row.name ?? "",
      enabled: row.enabled ?? false,
      provider: row.provider ?? "",
      source: row.source ?? "",
      color: row.color ?? "",
      connection_id: row.connection_id ?? "",
      created_at: row.created_at ?? "",
    };
  }, [id, row]);
}

/**
 * Toggle a calendar's `enabled` flag through the Rust plugin. The command is
 * the single-writer path for `calendars.json` — never call
 * `store.setPartialRow("calendars", ...)` directly, since the TinyBase
 * persister for this table is load-only (see
 * `apps/desktop/src/store/tinybase/persister/calendar/persister.ts`) and the
 * Rust sync worker would clobber any UI-side writes on its next pass.
 */
export function useSetCalendarEnabled() {
  return useCallback(
    async (calendarId: string, enabled: boolean): Promise<void> => {
      const result = await calendarCommands.setCalendarEnabled(
        calendarId,
        enabled,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
    [],
  );
}

export type EnabledCalendar = { id: string; provider: string };

export function useEnabledCalendars(): EnabledCalendar[] {
  const resultTable = main.UI.useResultTable(
    main.QUERIES.enabledCalendars,
    main.STORE_ID,
  );
  return useMemo(() => {
    if (!resultTable) return [];
    return Object.entries(resultTable).map(([id, row]) => ({
      id,
      provider: row.provider ?? "",
    }));
  }, [resultTable]);
}

export type CalendarData = {
  eventIdsByDate: Record<string, string[]>;
  sessionIdsByDate: Record<string, string[]>;
};

function compareNullableDates(a: string | undefined, b: string | undefined) {
  const aDate = a ? safeParseDate(a) : null;
  const bDate = b ? safeParseDate(b) : null;

  if (aDate && bDate) {
    return aDate.getTime() - bDate.getTime();
  }
  if (aDate) return -1;
  if (bDate) return 1;
  return 0;
}

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
          const allDayCompare = aAllDay - bAllDay;
          if (allDayCompare !== 0) return allDayCompare;

          const startCompare = compareNullableDates(
            eventsTable[a]?.started_at as string | undefined,
            eventsTable[b]?.started_at as string | undefined,
          );
          if (startCompare !== 0) return startCompare;

          const titleCompare = String(
            eventsTable[a]?.title ?? "",
          ).localeCompare(String(eventsTable[b]?.title ?? ""));
          if (titleCompare !== 0) return titleCompare;

          return a.localeCompare(b);
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

      for (const ids of Object.values(sessionIdsByDate)) {
        ids.sort((a, b) => {
          const createdAtCompare = compareNullableDates(
            sessionsTable[a]?.created_at as string | undefined,
            sessionsTable[b]?.created_at as string | undefined,
          );
          if (createdAtCompare !== 0) return createdAtCompare;

          const titleCompare = String(
            sessionsTable[a]?.title ?? "",
          ).localeCompare(String(sessionsTable[b]?.title ?? ""));
          if (titleCompare !== 0) return titleCompare;

          return a.localeCompare(b);
        });
      }
    }

    return { eventIdsByDate, sessionIdsByDate };
  }, [eventsTable, sessionsTable, tz, isIgnored]);
}
