import type { EventParticipant } from "@hypr/store";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
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

type CalendarEventStartSqlRow = { started_at: string };

type CalendarEventSearchSqlRow = {
  id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location: string;
  meeting_link: string;
  description: string;
  participant_count: number;
  linked_session_id: string;
};

export type CalendarEventSearchResult = {
  id: string;
  title: string;
  startedAt: string | null;
  endedAt: string | null;
  location: string | null;
  meetingLink: string | null;
  description: string | null;
  participantCount: number;
  linkedSessionId: string | null;
};

type NearbyCalendarEventSqlRow = {
  id: string;
  title: string;
  started_at: string;
  meeting_link: string;
  location: string;
  description: string;
  participants_json: string | null;
};

export type NearbyCalendarEvent = {
  id: string;
  title: string;
  meetingLink?: string;
  location?: string;
  description?: string;
  participantNames: string[];
};

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
  const timelineEventsTable = useTimelineEventsTable();
  const timelineSessionsTable = useTimelineSessionsTable();

  return { timelineEventsTable, timelineSessionsTable };
}

export function useTimelineEventsTable(): TimelineEventsTable {
  const { data: timelineEventsTable = EMPTY_EVENTS } = useLiveQuery<
    TimelineEventSqlRow,
    Record<string, TimelineEventRow>
  >({
    sql: `
      SELECT
        event.id,
        event.title,
        event.started_at,
        event.ended_at,
        event.calendar_id,
        event.tracking_id_event,
        event.has_recurrence_rules,
        event.recurrence_series_id,
        event.is_all_day,
        event.location,
        event.meeting_link,
        event.description,
        calendar.color AS calendar_color
      FROM events AS event
      LEFT JOIN calendars AS calendar
        ON calendar.id = event.calendar_id AND calendar.deleted_at IS NULL
      WHERE event.deleted_at IS NULL
      ORDER BY event.started_at, event.id
    `,
    mapRows: mapTimelineEventRows,
  });

  return timelineEventsTable;
}

export function useTimelineSessionsTable(): TimelineSessionsTable {
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

  return timelineSessionsTable;
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

export function useCalendarRows(provider?: string): CalendarRow[] {
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
      WHERE deleted_at IS NULL AND (? = '' OR provider = ?)
      ORDER BY name, id
    `,
    params: [provider ?? "", provider ?? ""],
    mapRows: (rows) => rows.map(normalizeCalendarRow),
  });

  return data;
}

export function setCalendarEnabled(
  calendarId: string,
  enabled: boolean,
): Promise<void> {
  return enqueueDatabaseWrite(`calendar-selection:${calendarId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE calendars
          SET enabled = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [Number(enabled), now, calendarId],
      },
      {
        sql: `
          UPDATE events
          SET deleted_at = ?, updated_at = ?
          WHERE calendar_id = ? AND deleted_at IS NULL AND ? = 0
        `,
        params: [now, now, calendarId, Number(enabled)],
      },
    ]);
  });
}

export async function getCalendarEventStartedAt(
  eventId: string,
): Promise<string | null> {
  const rows = await liveQueryClient.execute<CalendarEventStartSqlRow>(
    `
      SELECT started_at
      FROM events
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [eventId],
  );
  return rows[0]?.started_at || null;
}

export async function searchCalendarEvents(
  query: string,
  limit: number,
): Promise<CalendarEventSearchResult[]> {
  const normalizedQuery = query.trim().toLowerCase();
  const rows = await liveQueryClient.execute<CalendarEventSearchSqlRow>(
    `
      SELECT
        event.id,
        event.title,
        event.started_at,
        event.ended_at,
        event.location,
        event.meeting_link,
        event.description,
        CASE
          WHEN json_valid(event.participants_json)
            AND json_type(event.participants_json) = 'array'
          THEN json_array_length(event.participants_json)
          ELSE 0
        END AS participant_count,
        COALESCE((
          SELECT session.id
          FROM sessions AS session
          WHERE session.deleted_at IS NULL
            AND (
              session.event_id = event.id
              OR (
                event.tracking_id_event <> ''
                AND session.external_event_id = event.tracking_id_event
              )
              OR (
                json_valid(session.event_json)
                AND json_extract(session.event_json, '$.tracking_id') =
                  event.tracking_id_event
                AND json_extract(session.event_json, '$.calendar_id') =
                  event.calendar_id
              )
            )
          ORDER BY session.created_at, session.id
          LIMIT 1
        ), '') AS linked_session_id
      FROM events AS event
      WHERE event.deleted_at IS NULL
        AND (
          ? = ''
          OR instr(
            lower(
              event.title || char(10) ||
              event.location || char(10) ||
              event.meeting_link || char(10) ||
              event.description
            ),
            ?
          ) > 0
        )
      ORDER BY julianday(event.started_at) DESC, event.id
      LIMIT ?
    `,
    [normalizedQuery, normalizedQuery, limit],
  );

  return rows.map((row) => ({
    id: row.id,
    title: row.title || "Untitled event",
    startedAt: row.started_at || null,
    endedAt: row.ended_at || null,
    location: row.location || null,
    meetingLink: row.meeting_link || null,
    description: row.description || null,
    participantCount: Number(row.participant_count) || 0,
    linkedSessionId: row.linked_session_id || null,
  }));
}

export async function getNearbyCalendarEvents(
  nowMs: number,
  windowMs: number,
): Promise<NearbyCalendarEvent[]> {
  const rows = await liveQueryClient.execute<NearbyCalendarEventSqlRow>(
    `
      SELECT
        id,
        title,
        started_at,
        meeting_link,
        location,
        description,
        participants_json
      FROM events
      WHERE deleted_at IS NULL
        AND is_all_day = 0
        AND started_at <> ''
        AND abs(
          ((julianday(started_at) - 2440587.5) * 86400000) - ?
        ) <= ?
      ORDER BY abs(
        ((julianday(started_at) - 2440587.5) * 86400000) - ?
      ), julianday(started_at), id
    `,
    [nowMs, windowMs, nowMs],
  );

  return rows.map((row) => ({
    id: row.id,
    title: row.title || "Untitled Event",
    meetingLink: row.meeting_link || undefined,
    location: row.location || undefined,
    description: row.description || undefined,
    participantNames: [
      ...new Set(
        parseEventParticipants(row.participants_json ?? undefined)
          .filter((participant) => participant.is_current_user !== true)
          .map((participant) => participant.name?.trim() ?? "")
          .filter(Boolean),
      ),
    ],
  }));
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
