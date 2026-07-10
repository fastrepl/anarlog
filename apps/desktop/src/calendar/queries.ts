import type { EventParticipant } from "@hypr/store";

import { useLiveQuery } from "~/db";
import type {
  TimelineEventRow,
  TimelineEventsTable,
  TimelineSessionRow,
  TimelineSessionsTable,
} from "~/sidebar/timeline/utils";

type TimelineEventSqlRow = Omit<
  TimelineEventRow,
  "has_recurrence_rules" | "is_all_day"
> & {
  id: string;
  has_recurrence_rules: boolean | number;
  is_all_day: boolean | number;
};

type TimelineSessionSqlRow = TimelineSessionRow & { id: string };

type CalendarSqlRow = {
  id: string;
  tracking_id_calendar: string;
  name: string;
  enabled: boolean | number;
  provider: string;
  source: string;
  color: string;
  connection_id: string;
  created_at: string;
};

type EventParticipantsSqlRow = { participants_json: string };

export type CalendarRow = Omit<CalendarSqlRow, "enabled"> & {
  enabled: boolean;
};

const EMPTY_EVENTS: Record<string, TimelineEventRow> = {};
const EMPTY_SESSIONS: Record<string, TimelineSessionRow> = {};
const EMPTY_CALENDARS: CalendarRow[] = [];
const EMPTY_EVENT_PARTICIPANTS: EventParticipant[] = [];

export function useTimelineTables(): {
  timelineEventsTable: TimelineEventsTable;
  timelineSessionsTable: TimelineSessionsTable;
} {
  const { data: timelineEventsTable = EMPTY_EVENTS } = useLiveQuery<
    TimelineEventSqlRow,
    Record<string, TimelineEventRow>
  >({
    sql: `
      SELECT
        id,
        title,
        started_at,
        ended_at,
        calendar_id,
        tracking_id_event,
        has_recurrence_rules,
        recurrence_series_id,
        is_all_day
      FROM events
      WHERE deleted_at IS NULL
      ORDER BY started_at, id
    `,
    mapRows: mapTimelineEventRows,
  });
  const { data: timelineSessionsTable = EMPTY_SESSIONS } = useLiveQuery<
    TimelineSessionSqlRow,
    Record<string, TimelineSessionRow>
  >({
    sql: `
      SELECT
        id,
        title,
        created_at,
        event_json,
        folder_path AS folder_id
      FROM sessions
      WHERE deleted_at IS NULL
      ORDER BY created_at, id
    `,
    mapRows: mapTimelineSessionRows,
  });

  return { timelineEventsTable, timelineSessionsTable };
}

export function useCalendarRow(
  id: string | null | undefined,
): CalendarRow | null {
  const { data = null } = useLiveQuery<CalendarSqlRow, CalendarRow | null>({
    sql: `
      SELECT
        id,
        tracking_id_calendar,
        name,
        enabled,
        provider,
        source,
        color,
        connection_id,
        created_at
      FROM calendars
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [id ?? ""],
    enabled: Boolean(id),
    mapRows: (rows) => {
      const row = rows[0];
      return row ? normalizeCalendarRow(row) : null;
    },
  });

  return id ? data : null;
}

export function useEnabledCalendarRows(): CalendarRow[] {
  const { data = EMPTY_CALENDARS } = useLiveQuery<
    CalendarSqlRow,
    CalendarRow[]
  >({
    sql: `
      SELECT
        id,
        tracking_id_calendar,
        name,
        enabled,
        provider,
        source,
        color,
        connection_id,
        created_at
      FROM calendars
      WHERE enabled = 1 AND deleted_at IS NULL
      ORDER BY name, id
    `,
    mapRows: (rows) => rows.map(normalizeCalendarRow),
  });

  return data;
}

export function useSessionEventParticipants(
  sessionId: string,
): EventParticipant[] {
  const { data = EMPTY_EVENT_PARTICIPANTS } = useLiveQuery<
    EventParticipantsSqlRow,
    EventParticipant[]
  >({
    sql: `
      SELECT event.participants_json
      FROM sessions AS session
      JOIN events AS event
        ON event.deleted_at IS NULL
        AND (
          event.id = session.event_id
          OR (
            event.tracking_id_event = CASE
              WHEN json_valid(session.event_json)
              THEN json_extract(session.event_json, '$.tracking_id')
              ELSE ''
            END
            AND event.calendar_id = CASE
              WHEN json_valid(session.event_json)
              THEN json_extract(session.event_json, '$.calendar_id')
              ELSE ''
            END
          )
        )
      WHERE session.id = ? AND session.deleted_at IS NULL
      ORDER BY event.started_at, event.id
      LIMIT 1
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => parseEventParticipants(rows[0]?.participants_json),
  });
  return sessionId ? data : EMPTY_EVENT_PARTICIPANTS;
}

export function mapTimelineEventRows(
  rows: TimelineEventSqlRow[],
): Record<string, TimelineEventRow> {
  return Object.fromEntries(
    rows.map(({ id, ...row }) => [
      id,
      {
        ...row,
        has_recurrence_rules: Boolean(row.has_recurrence_rules),
        is_all_day: Boolean(row.is_all_day),
      },
    ]),
  );
}

export function mapTimelineSessionRows(
  rows: TimelineSessionSqlRow[],
): Record<string, TimelineSessionRow> {
  return Object.fromEntries(rows.map(({ id, ...row }) => [id, row]));
}

function normalizeCalendarRow(row: CalendarSqlRow): CalendarRow {
  return { ...row, enabled: Boolean(row.enabled) };
}

export function parseEventParticipants(
  value: string | undefined,
): EventParticipant[] {
  if (!value) return EMPTY_EVENT_PARTICIPANTS;

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? (parsed as EventParticipant[])
      : EMPTY_EVENT_PARTICIPANTS;
  } catch {
    return EMPTY_EVENT_PARTICIPANTS;
  }
}
