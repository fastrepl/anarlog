import { format } from "date-fns";
import { useCallback, useEffect, useMemo, useState } from "react";

import type { IgnoredEvent, IgnoredRecurringSeries } from "@hypr/store";
import { safeParseDate } from "@hypr/utils";
import { TZDate } from "@hypr/utils";

import { useConfigValue } from "~/shared/config";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";

// Storage boundary for calendar/event reads and writes consumed by UI.
// Keep return values plain TS objects so backend swaps stay transparent.
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

export const ENABLED_CALENDARS_QUERY = main.QUERIES.enabledCalendars;

export function useCalendarsByProvider(provider: string): Calendar[] {
  const table = main.UI.useTable("calendars", main.STORE_ID);
  return useMemo(() => {
    const out: Calendar[] = [];
    for (const [id, row] of Object.entries(table)) {
      if (row.provider !== provider) continue;
      out.push({
        id,
        tracking_id_calendar: (row.tracking_id_calendar as string) ?? "",
        name: (row.name as string) ?? "",
        enabled: (row.enabled as boolean) ?? false,
        provider: (row.provider as string) ?? "",
        source: (row.source as string) ?? "",
        color: (row.color as string) ?? "",
        connection_id: (row.connection_id as string) ?? "",
        created_at: (row.created_at as string) ?? "",
      });
    }
    return out;
  }, [table, provider]);
}

export function useSetCalendarEnabled(): (
  calendarId: string,
  enabled: boolean,
) => void {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (calendarId, enabled) => {
      if (!store) return;
      store.setPartialRow("calendars", calendarId, { enabled });
    },
    [store],
  );
}

export type EventDetails = {
  title: string | undefined;
  startedAt: string | undefined;
  endedAt: string | undefined;
  location: string | undefined;
  meetingLink: string | undefined;
  description: string | undefined;
  calendarId: string | undefined;
};

export function useEvent(eventId: string | undefined): EventDetails | null {
  const title = main.UI.useCell(
    "events",
    eventId ?? "",
    "title",
    main.STORE_ID,
  );
  const startedAt = main.UI.useCell(
    "events",
    eventId ?? "",
    "started_at",
    main.STORE_ID,
  );
  const endedAt = main.UI.useCell(
    "events",
    eventId ?? "",
    "ended_at",
    main.STORE_ID,
  );
  const location = main.UI.useCell(
    "events",
    eventId ?? "",
    "location",
    main.STORE_ID,
  );
  const meetingLink = main.UI.useCell(
    "events",
    eventId ?? "",
    "meeting_link",
    main.STORE_ID,
  );
  const description = main.UI.useCell(
    "events",
    eventId ?? "",
    "description",
    main.STORE_ID,
  );
  const calendarId = main.UI.useCell(
    "events",
    eventId ?? "",
    "calendar_id",
    main.STORE_ID,
  );

  return useMemo(
    () =>
      eventId
        ? {
            title,
            startedAt,
            endedAt,
            location,
            meetingLink,
            description,
            calendarId,
          }
        : null,
    [
      eventId,
      title,
      startedAt,
      endedAt,
      location,
      meetingLink,
      description,
      calendarId,
    ],
  );
}

function parseIgnoredEvents(raw: string | undefined): IgnoredEvent[] {
  if (!raw) return [];
  try {
    return JSON.parse(raw) as IgnoredEvent[];
  } catch {
    return [];
  }
}

function parseIgnoredSeries(raw: string | undefined): IgnoredRecurringSeries[] {
  if (!raw) return [];
  try {
    return JSON.parse(raw) as IgnoredRecurringSeries[];
  } catch {
    return [];
  }
}

export function useIgnoredEvents() {
  const store = main.UI.useStore(main.STORE_ID);

  const ignoredEventsRaw = main.UI.useValue("ignored_events", main.STORE_ID) as
    | string
    | undefined;
  const ignoredSeriesRaw = main.UI.useValue(
    "ignored_recurring_series",
    main.STORE_ID,
  ) as string | undefined;

  const ignoredIds = useMemo(() => {
    const list = parseIgnoredEvents(ignoredEventsRaw);
    return new Set(list.map((e) => e.tracking_id));
  }, [ignoredEventsRaw]);

  const ignoredSeriesIds = useMemo(() => {
    const list = parseIgnoredSeries(ignoredSeriesRaw);
    return new Set(list.map((e) => e.id));
  }, [ignoredSeriesRaw]);

  const isIgnored = useCallback(
    (
      trackingId: string | null | undefined,
      recurrenceSeriesId: string | null | undefined,
    ) => {
      if (!trackingId) return false;
      if (ignoredIds.has(trackingId)) return true;
      if (recurrenceSeriesId && ignoredSeriesIds.has(recurrenceSeriesId))
        return true;
      return false;
    },
    [ignoredIds, ignoredSeriesIds],
  );

  const ignoreEvent = useCallback(
    (trackingId: string) => {
      if (!store) return;
      const list = parseIgnoredEvents(
        store.getValue("ignored_events") as string | undefined,
      );
      const now = new Date().toISOString();
      list.push({
        tracking_id: trackingId,
        last_seen: now,
      });
      store.setValue("ignored_events", JSON.stringify(list));
    },
    [store],
  );

  const unignoreEvent = useCallback(
    (trackingId: string) => {
      if (!store) return;
      const list = parseIgnoredEvents(
        store.getValue("ignored_events") as string | undefined,
      );
      const filtered = list.filter((e) => e.tracking_id !== trackingId);
      store.setValue("ignored_events", JSON.stringify(filtered));
    },
    [store],
  );

  const ignoreSeries = useCallback(
    (seriesId: string) => {
      if (!store) return;
      const list = parseIgnoredSeries(
        store.getValue("ignored_recurring_series") as string | undefined,
      );
      if (!list.some((e) => e.id === seriesId)) {
        list.push({ id: seriesId, last_seen: new Date().toISOString() });
        store.setValue("ignored_recurring_series", JSON.stringify(list));
      }
    },
    [store],
  );

  const unignoreSeries = useCallback(
    (seriesId: string) => {
      if (!store) return;
      const list = parseIgnoredSeries(
        store.getValue("ignored_recurring_series") as string | undefined,
      );
      store.setValue(
        "ignored_recurring_series",
        JSON.stringify(list.filter((e) => e.id !== seriesId)),
      );
    },
    [store],
  );

  return {
    isIgnored,
    ignoreEvent,
    unignoreEvent,
    ignoreSeries,
    unignoreSeries,
  };
}

export type TimelineEvent = {
  id: string;
  title: string;
  started_at: string;
  ended_at: string;
  calendar_id: string;
  tracking_id_event: string;
  has_recurrence_rules: boolean;
  recurrence_series_id: string;
  is_all_day: boolean;
};

export function useTimelineEvent(eventId: string): TimelineEvent | null {
  const row = main.UI.useResultRow(
    main.QUERIES.timelineEvents,
    eventId,
    main.STORE_ID,
  );
  return useMemo(() => {
    if (!row || Object.keys(row).length === 0) return null;
    return {
      id: eventId,
      title: (row.title as string) ?? "",
      started_at: (row.started_at as string) ?? "",
      ended_at: (row.ended_at as string) ?? "",
      calendar_id: (row.calendar_id as string) ?? "",
      tracking_id_event: (row.tracking_id_event as string) ?? "",
      has_recurrence_rules: (row.has_recurrence_rules as boolean) ?? false,
      recurrence_series_id: (row.recurrence_series_id as string) ?? "",
      is_all_day: (row.is_all_day as boolean) ?? false,
    };
  }, [eventId, row]);
}

export type TimelineSession = {
  id: string;
  title: string;
  created_at: string;
  event_json: string;
  folder_id: string;
};

export function useGetOrCreateSessionForEventId(): (
  eventId: string,
  title: string,
) => string | null {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (eventId, title) => {
      if (!store) return null;
      return getOrCreateSessionForEventId(store, eventId, title);
    },
    [store],
  );
}

export function useTimelineSession(sessionId: string): TimelineSession | null {
  const row = main.UI.useResultRow(
    main.QUERIES.timelineSessions,
    sessionId,
    main.STORE_ID,
  );
  return useMemo(() => {
    if (!row || Object.keys(row).length === 0) return null;
    return {
      id: sessionId,
      title: (row.title as string) ?? "",
      created_at: (row.created_at as string) ?? "",
      event_json: (row.event_json as string) ?? "",
      folder_id: (row.folder_id as string) ?? "",
    };
  }, [sessionId, row]);
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
