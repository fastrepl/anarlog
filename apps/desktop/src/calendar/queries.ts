import { useMutation } from "@tanstack/react-query";
import { useCallback } from "react";

import { and, calendars, eq, events, gte, inArray, lte, sql } from "@hypr/db";

import { db, useDrizzleLiveQuery } from "~/db";

// ---------------------------------------------------------------------------
// Live-row types (raw SQLite column names, snake_case)
// ---------------------------------------------------------------------------

type CalendarLiveRow = {
  id: string;
  tracking_id_calendar: string;
  name: string;
  enabled: number | boolean;
  provider: string;
  source: string;
  color: string;
  connection_id: string;
  created_at: string;
  updated_at: string;
};

type EventLiveRow = {
  id: string;
  tracking_id_event: string;
  calendar_id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location: string;
  meeting_link: string;
  description: string;
  note: string;
  recurrence_series_id: string;
  has_recurrence_rules: number | boolean;
  is_all_day: number | boolean;
  provider: string;
  participants_json: unknown;
  created_at: string;
  updated_at: string;
};

// ---------------------------------------------------------------------------
// Domain types (camelCase, parsed)
// ---------------------------------------------------------------------------

export type CalendarRecord = {
  id: string;
  trackingIdCalendar: string;
  name: string;
  enabled: boolean;
  provider: string;
  source: string;
  color: string;
  connectionId: string;
  createdAt: string;
};

export type EventRecord = {
  id: string;
  trackingIdEvent: string;
  calendarId: string;
  title: string;
  startedAt: string;
  endedAt: string;
  location: string;
  meetingLink: string;
  description: string;
  note: string;
  recurrenceSeriesId: string;
  hasRecurrenceRules: boolean;
  isAllDay: boolean;
  provider: string;
  participantsJson: unknown;
  createdAt: string;
};

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

function mapCalendarLiveRow(row: CalendarLiveRow): CalendarRecord {
  return {
    id: row.id,
    trackingIdCalendar: row.tracking_id_calendar,
    name: row.name,
    enabled: Boolean(row.enabled),
    provider: row.provider,
    source: row.source,
    color: row.color,
    connectionId: row.connection_id,
    createdAt: row.created_at,
  };
}

function mapCalendarLiveRows(rows: CalendarLiveRow[]): CalendarRecord[] {
  return rows.map(mapCalendarLiveRow);
}

function mapEventLiveRow(row: EventLiveRow): EventRecord {
  return {
    id: row.id,
    trackingIdEvent: row.tracking_id_event,
    calendarId: row.calendar_id,
    title: row.title,
    startedAt: row.started_at,
    endedAt: row.ended_at,
    location: row.location,
    meetingLink: row.meeting_link,
    description: row.description,
    note: row.note,
    recurrenceSeriesId: row.recurrence_series_id,
    hasRecurrenceRules: Boolean(row.has_recurrence_rules),
    isAllDay: Boolean(row.is_all_day),
    provider: row.provider,
    participantsJson: row.participants_json,
    createdAt: row.created_at,
  };
}

function mapEventLiveRows(rows: EventLiveRow[]): EventRecord[] {
  return rows.map(mapEventLiveRow);
}

// ---------------------------------------------------------------------------
// Reactive hooks — calendars
// ---------------------------------------------------------------------------

export function useCalendars(): CalendarRecord[] {
  const query = db.select().from(calendars).orderBy(calendars.name);
  const { data = [] } = useDrizzleLiveQuery<CalendarLiveRow, CalendarRecord[]>(
    query,
    { mapRows: mapCalendarLiveRows },
  );
  return data;
}

export function useEnabledCalendars(): CalendarRecord[] {
  const query = db
    .select()
    .from(calendars)
    .where(eq(calendars.enabled, true))
    .orderBy(calendars.name);
  const { data = [] } = useDrizzleLiveQuery<CalendarLiveRow, CalendarRecord[]>(
    query,
    { mapRows: mapCalendarLiveRows },
  );
  return data;
}

export function useCalendarById(
  id: string | null | undefined,
): CalendarRecord | null {
  const query = db
    .select()
    .from(calendars)
    .where(eq(calendars.id, id ?? ""))
    .limit(1);
  const { data = null } = useDrizzleLiveQuery<
    CalendarLiveRow,
    CalendarRecord | null
  >(query, {
    mapRows: (rows) => (rows.length > 0 ? mapCalendarLiveRow(rows[0]) : null),
  });
  return data;
}

// ---------------------------------------------------------------------------
// Reactive hooks — events
// ---------------------------------------------------------------------------

export function useEventById(eventId: string | undefined): EventRecord | null {
  const query = db
    .select()
    .from(events)
    .where(eq(events.id, eventId ?? ""))
    .limit(1);
  const { data = null } = useDrizzleLiveQuery<EventLiveRow, EventRecord | null>(
    query,
    {
      mapRows: (rows) => (rows.length > 0 ? mapEventLiveRow(rows[0]) : null),
      enabled: !!eventId,
    },
  );
  return data;
}

export function useAllEvents(): EventRecord[] {
  const query = db.select().from(events).orderBy(events.startedAt);
  const { data = [] } = useDrizzleLiveQuery<EventLiveRow, EventRecord[]>(
    query,
    { mapRows: mapEventLiveRows },
  );
  return data;
}

type TimelineEventRow = {
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

export function useTimelineEvents(): Record<string, TimelineEventRow> | null {
  const query = db.select().from(events);
  const { data = null } = useDrizzleLiveQuery<
    EventLiveRow,
    Record<string, TimelineEventRow> | null
  >(query, {
    mapRows: (rows) => {
      if (rows.length === 0) return null;
      const result: Record<string, TimelineEventRow> = {};
      for (const row of rows) {
        result[row.id] = {
          id: row.id,
          title: row.title,
          started_at: row.started_at,
          ended_at: row.ended_at,
          calendar_id: row.calendar_id,
          tracking_id_event: row.tracking_id_event,
          has_recurrence_rules: Boolean(row.has_recurrence_rules),
          recurrence_series_id: row.recurrence_series_id,
          is_all_day: Boolean(row.is_all_day),
        };
      }
      return result;
    },
  });
  return data;
}

// ---------------------------------------------------------------------------
// Imperative reads
// ---------------------------------------------------------------------------

export async function getEnabledCalendars(): Promise<CalendarRecord[]> {
  const rows = await db
    .select()
    .from(calendars)
    .where(eq(calendars.enabled, true));
  return rows.map((row) => ({
    id: row.id,
    trackingIdCalendar: row.trackingIdCalendar,
    name: row.name,
    enabled: row.enabled,
    provider: row.provider,
    source: row.source,
    color: row.color,
    connectionId: row.connectionId,
    createdAt: row.createdAt,
  }));
}

export async function getAllCalendars(): Promise<CalendarRecord[]> {
  const rows = await db.select().from(calendars);
  return rows.map((row) => ({
    id: row.id,
    trackingIdCalendar: row.trackingIdCalendar,
    name: row.name,
    enabled: row.enabled,
    provider: row.provider,
    source: row.source,
    color: row.color,
    connectionId: row.connectionId,
    createdAt: row.createdAt,
  }));
}

export async function getCalendarById(
  id: string,
): Promise<CalendarRecord | null> {
  const rows = await db
    .select()
    .from(calendars)
    .where(eq(calendars.id, id))
    .limit(1);
  if (rows.length === 0) return null;
  const row = rows[0];
  return {
    id: row.id,
    trackingIdCalendar: row.trackingIdCalendar,
    name: row.name,
    enabled: row.enabled,
    provider: row.provider,
    source: row.source,
    color: row.color,
    connectionId: row.connectionId,
    createdAt: row.createdAt,
  };
}

export async function getEventById(id: string): Promise<EventRecord | null> {
  const rows = await db.select().from(events).where(eq(events.id, id)).limit(1);
  if (rows.length === 0) return null;
  const row = rows[0];
  return {
    id: row.id,
    trackingIdEvent: row.trackingIdEvent,
    calendarId: row.calendarId,
    title: row.title,
    startedAt: row.startedAt,
    endedAt: row.endedAt,
    location: row.location,
    meetingLink: row.meetingLink,
    description: row.description,
    note: row.note,
    recurrenceSeriesId: row.recurrenceSeriesId,
    hasRecurrenceRules: row.hasRecurrenceRules,
    isAllDay: row.isAllDay,
    provider: row.provider,
    participantsJson: row.participantsJson,
    createdAt: row.createdAt,
  };
}

export async function eventExists(id: string): Promise<boolean> {
  const rows = await db
    .select({ id: events.id })
    .from(events)
    .where(eq(events.id, id))
    .limit(1);
  return rows.length > 0;
}

export async function getEventsByCalendarIds(
  calendarIds: string[],
  from: string,
  to: string,
): Promise<EventRecord[]> {
  if (calendarIds.length === 0) return [];
  const rows = await db
    .select()
    .from(events)
    .where(
      and(
        inArray(events.calendarId, calendarIds),
        gte(events.startedAt, from),
        lte(events.startedAt, to),
      ),
    );
  return rows.map((row) => ({
    id: row.id,
    trackingIdEvent: row.trackingIdEvent,
    calendarId: row.calendarId,
    title: row.title,
    startedAt: row.startedAt,
    endedAt: row.endedAt,
    location: row.location,
    meetingLink: row.meetingLink,
    description: row.description,
    note: row.note,
    recurrenceSeriesId: row.recurrenceSeriesId,
    hasRecurrenceRules: row.hasRecurrenceRules,
    isAllDay: row.isAllDay,
    provider: row.provider,
    participantsJson: row.participantsJson,
    createdAt: row.createdAt,
  }));
}

export async function getAllEvents(): Promise<EventRecord[]> {
  const rows = await db.select().from(events);
  return rows.map((row) => ({
    id: row.id,
    trackingIdEvent: row.trackingIdEvent,
    calendarId: row.calendarId,
    title: row.title,
    startedAt: row.startedAt,
    endedAt: row.endedAt,
    location: row.location,
    meetingLink: row.meetingLink,
    description: row.description,
    note: row.note,
    recurrenceSeriesId: row.recurrenceSeriesId,
    hasRecurrenceRules: row.hasRecurrenceRules,
    isAllDay: row.isAllDay,
    provider: row.provider,
    participantsJson: row.participantsJson,
    createdAt: row.createdAt,
  }));
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

export async function insertCalendar(cal: {
  id: string;
  trackingIdCalendar: string;
  name: string;
  enabled: boolean;
  provider: string;
  source: string;
  color: string;
  connectionId: string;
  createdAt?: string;
}): Promise<void> {
  await db.insert(calendars).values({
    id: cal.id,
    trackingIdCalendar: cal.trackingIdCalendar,
    name: cal.name,
    enabled: cal.enabled,
    provider: cal.provider,
    source: cal.source,
    color: cal.color,
    connectionId: cal.connectionId,
    createdAt: cal.createdAt ?? sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
    updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
  });
}

export async function updateCalendar(
  id: string,
  fields: Partial<{
    trackingIdCalendar: string;
    name: string;
    enabled: boolean;
    provider: string;
    source: string;
    color: string;
    connectionId: string;
  }>,
): Promise<void> {
  await db
    .update(calendars)
    .set({
      ...fields,
      updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
    })
    .where(eq(calendars.id, id));
}

export async function deleteCalendar(id: string): Promise<void> {
  await db.delete(calendars).where(eq(calendars.id, id));
}

export async function insertEvent(evt: {
  id: string;
  trackingIdEvent: string;
  calendarId: string;
  title: string;
  startedAt: string;
  endedAt: string;
  location: string;
  meetingLink: string;
  description: string;
  note?: string;
  recurrenceSeriesId: string;
  hasRecurrenceRules: boolean;
  isAllDay: boolean;
  provider: string;
  participantsJson?: unknown;
  createdAt?: string;
}): Promise<void> {
  await db.insert(events).values({
    id: evt.id,
    trackingIdEvent: evt.trackingIdEvent,
    calendarId: evt.calendarId,
    title: evt.title,
    startedAt: evt.startedAt,
    endedAt: evt.endedAt,
    location: evt.location,
    meetingLink: evt.meetingLink,
    description: evt.description,
    note: evt.note ?? "",
    recurrenceSeriesId: evt.recurrenceSeriesId,
    hasRecurrenceRules: evt.hasRecurrenceRules,
    isAllDay: evt.isAllDay,
    provider: evt.provider,
    participantsJson: evt.participantsJson ?? null,
    createdAt: evt.createdAt ?? sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
    updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
  });
}

export async function updateEvent(
  id: string,
  fields: Partial<{
    trackingIdEvent: string;
    calendarId: string;
    title: string;
    startedAt: string;
    endedAt: string;
    location: string;
    meetingLink: string;
    description: string;
    note: string;
    recurrenceSeriesId: string;
    hasRecurrenceRules: boolean;
    isAllDay: boolean;
    provider: string;
    participantsJson: unknown;
  }>,
): Promise<void> {
  await db
    .update(events)
    .set({
      ...fields,
      updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
    })
    .where(eq(events.id, id));
}

export async function deleteEvent(id: string): Promise<void> {
  await db.delete(events).where(eq(events.id, id));
}

export async function deleteEventsByCalendarId(
  calendarId: string,
): Promise<void> {
  await db.delete(events).where(eq(events.calendarId, calendarId));
}

// ---------------------------------------------------------------------------
// Mutation hooks
// ---------------------------------------------------------------------------

export function useToggleCalendarEnabled() {
  const mutation = useMutation({
    mutationFn: async ({ id, enabled }: { id: string; enabled: boolean }) => {
      await updateCalendar(id, { enabled });
    },
  });

  return useCallback(
    (id: string, enabled: boolean) => mutation.mutateAsync({ id, enabled }),
    [mutation],
  );
}
