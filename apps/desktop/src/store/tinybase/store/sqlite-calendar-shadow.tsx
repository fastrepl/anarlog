import { calendars, eq, events, sql } from "@hypr/db";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteCalendarRow = {
  id: string;
  tracking_id_calendar: string;
  name: string;
  enabled: number | boolean;
  provider: string;
  source: string;
  color: string;
  connection_id: string;
  created_at: string;
  deleted_at: string | null;
};

type SqliteEventRow = {
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
  deleted_at: string | null;
};

const CALENDAR_SHADOW_CONFIG: SqliteTableShadowConfig<
  "calendars",
  SqliteCalendarRow
> = {
  label: "SqliteCalendarShadow",
  tableId: "calendars",
  selectSql: `
    SELECT
      id,
      tracking_id_calendar,
      name,
      enabled,
      provider,
      source,
      color,
      connection_id,
      created_at,
      deleted_at
    FROM calendars
    ORDER BY name, id
  `,
  fromSqlite: (row) =>
    normalizeCalendarRow({
      user_id: "",
      created_at: row.created_at,
      tracking_id_calendar: row.tracking_id_calendar,
      name: row.name,
      enabled: Boolean(row.enabled),
      provider: row.provider,
      source: row.source,
      color: row.color,
      connection_id: row.connection_id,
    }),
  normalize: normalizeCalendarRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistCalendarChanges,
};

const EVENT_SHADOW_CONFIG: SqliteTableShadowConfig<"events", SqliteEventRow> = {
  label: "SqliteEventShadow",
  tableId: "events",
  selectSql: `
    SELECT
      id,
      tracking_id_event,
      calendar_id,
      title,
      started_at,
      ended_at,
      location,
      meeting_link,
      description,
      note,
      recurrence_series_id,
      has_recurrence_rules,
      is_all_day,
      provider,
      participants_json,
      created_at,
      deleted_at
    FROM events
    ORDER BY started_at, id
  `,
  fromSqlite: (row) =>
    normalizeEventRow({
      user_id: "",
      created_at: row.created_at,
      tracking_id_event: row.tracking_id_event,
      calendar_id: row.calendar_id,
      title: row.title,
      started_at: row.started_at,
      ended_at: row.ended_at,
      location: row.location,
      meeting_link: row.meeting_link,
      description: row.description,
      note: row.note,
      recurrence_series_id: row.recurrence_series_id,
      has_recurrence_rules: Boolean(row.has_recurrence_rules),
      is_all_day: Boolean(row.is_all_day),
      provider: row.provider,
      participants_json: encodeJson(row.participants_json),
    }),
  normalize: normalizeEventRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistEventChanges,
};

export function SqliteCalendarShadow({ store }: { store: Store }) {
  return (
    <>
      <SqliteTableShadow config={CALENDAR_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={EVENT_SHADOW_CONFIG} store={store} />
    </>
  );
}

export function normalizeCalendarRow(
  row: Partial<MainTableRow<"calendars">>,
): MainTableRow<"calendars"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    tracking_id_calendar:
      typeof row.tracking_id_calendar === "string"
        ? row.tracking_id_calendar
        : "",
    name: typeof row.name === "string" ? row.name : "",
    enabled: typeof row.enabled === "boolean" ? row.enabled : false,
    provider: typeof row.provider === "string" ? row.provider : "",
    source: typeof row.source === "string" ? row.source : "",
    color: typeof row.color === "string" ? row.color : "#888",
    connection_id:
      typeof row.connection_id === "string" ? row.connection_id : "",
  };
}

export function normalizeEventRow(
  row: Partial<MainTableRow<"events">>,
): MainTableRow<"events"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    tracking_id_event:
      typeof row.tracking_id_event === "string" ? row.tracking_id_event : "",
    calendar_id: typeof row.calendar_id === "string" ? row.calendar_id : "",
    title: typeof row.title === "string" ? row.title : "",
    started_at: typeof row.started_at === "string" ? row.started_at : "",
    ended_at: typeof row.ended_at === "string" ? row.ended_at : "",
    location: typeof row.location === "string" ? row.location : "",
    meeting_link: typeof row.meeting_link === "string" ? row.meeting_link : "",
    description: typeof row.description === "string" ? row.description : "",
    note: typeof row.note === "string" ? row.note : "",
    recurrence_series_id:
      typeof row.recurrence_series_id === "string"
        ? row.recurrence_series_id
        : "",
    has_recurrence_rules:
      typeof row.has_recurrence_rules === "boolean"
        ? row.has_recurrence_rules
        : false,
    is_all_day: typeof row.is_all_day === "boolean" ? row.is_all_day : false,
    provider: typeof row.provider === "string" ? row.provider : "",
    participants_json:
      typeof row.participants_json === "string" ? row.participants_json : "",
  };
}

async function persistCalendarChanges(
  rows: Array<[string, MainTableRow<"calendars">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(calendars)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeCalendarRow(value);
          return {
            id,
            trackingIdCalendar: row.tracking_id_calendar,
            name: row.name,
            enabled: row.enabled,
            provider: row.provider,
            source: row.source,
            color: row.color,
            connectionId: row.connection_id,
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: calendars.id,
        set: {
          trackingIdCalendar: sql`excluded.tracking_id_calendar`,
          name: sql`excluded.name`,
          enabled: sql`excluded.enabled`,
          provider: sql`excluded.provider`,
          source: sql`excluded.source`,
          color: sql`excluded.color`,
          connectionId: sql`excluded.connection_id`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(calendars)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(calendars.id, id));
  });
}

async function persistEventChanges(
  rows: Array<[string, MainTableRow<"events">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(events)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeEventRow(value);
          return {
            id,
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
            hasRecurrenceRules: row.has_recurrence_rules,
            isAllDay: row.is_all_day,
            provider: row.provider,
            participantsJson: parseJson(row.participants_json ?? ""),
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: events.id,
        set: {
          trackingIdEvent: sql`excluded.tracking_id_event`,
          calendarId: sql`excluded.calendar_id`,
          title: sql`excluded.title`,
          startedAt: sql`excluded.started_at`,
          endedAt: sql`excluded.ended_at`,
          location: sql`excluded.location`,
          meetingLink: sql`excluded.meeting_link`,
          description: sql`excluded.description`,
          note: sql`excluded.note`,
          recurrenceSeriesId: sql`excluded.recurrence_series_id`,
          hasRecurrenceRules: sql`excluded.has_recurrence_rules`,
          isAllDay: sql`excluded.is_all_day`,
          provider: sql`excluded.provider`,
          participantsJson: sql`excluded.participants_json`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(events)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(events.id, id));
  });
}

function encodeJson(value: unknown): string {
  if (typeof value === "string") return value;
  return value == null ? "" : JSON.stringify(value);
}

function parseJson(value: string): unknown {
  if (!value) return null;
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

async function softDelete(
  ids: string[],
  deleteRow: (id: string, deletedAt: string) => Promise<void>,
) {
  const deletedAt = new Date().toISOString();
  for (const id of ids) {
    await deleteRow(id, deletedAt);
  }
}

function chunks<T>(items: T[], size: number): T[][] {
  const result: T[][] = [];
  for (let index = 0; index < items.length; index += size) {
    result.push(items.slice(index, index + size));
  }
  return result;
}
