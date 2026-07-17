import type {
  CalendarListItem,
  CalendarProviderType,
} from "@hypr/plugin-calendar";

import type { Ctx } from "./ctx";
import type { ExistingEvent, IncomingParticipants } from "./fetch/types";
import type { SessionEventUpdate } from "./process/events/execute";
import type { EventsSyncOutput } from "./process/events/types";
import type { ParticipantsSyncOutput } from "./process/participants/types";

import { getCalendarTrackingKey } from "~/calendar/utils";
import { executeTransaction, liveQueryClient } from "~/db";
import { DEFAULT_USER_ID, id } from "~/shared/utils";

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
  deleted_at: string | null;
};

export type StoredCalendar = Omit<CalendarSqlRow, "enabled"> & {
  enabled: boolean;
};

type EventSqlRow = Omit<
  ExistingEvent,
  "has_recurrence_rules" | "is_all_day"
> & {
  has_recurrence_rules: boolean | number;
  is_all_day: boolean | number;
};

type SessionSqlRow = {
  id: string;
  owner_user_id: string;
  event_json: string;
  tracking_id: string;
};

export type SessionSyncRow = {
  id: string;
  ownerUserId: string;
  eventJson: string;
  trackingId: string;
};

type HumanSqlRow = {
  id: string;
  email: string;
};

export type ParticipantHuman = HumanSqlRow;

type ParticipantMappingSqlRow = {
  id: string;
  session_id: string;
  human_id: string;
  source: string;
};

export type ParticipantMapping = {
  id: string;
  sessionId: string;
  humanId: string;
  source: string;
};

export type ParticipantSyncSnapshot = {
  sessions: SessionSyncRow[];
  humans: ParticipantHuman[];
  mappings: ParticipantMapping[];
};

type Statement = { sql: string; params: unknown[] };

export async function loadEnabledCalendars(
  provider: CalendarProviderType,
  connectionId: string,
): Promise<StoredCalendar[]> {
  const rows = await liveQueryClient.execute<CalendarSqlRow>(
    `
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
      WHERE provider = ?
        AND connection_id = ?
        AND enabled = 1
        AND deleted_at IS NULL
      ORDER BY created_at, id
    `,
    [provider, connectionId],
  );

  return rows.map(normalizeCalendar);
}

export async function applyCalendarInventory({
  provider,
  requestedConnectionIds,
  successfulConnections,
}: {
  provider: CalendarProviderType;
  requestedConnectionIds: string[];
  successfulConnections: Array<{
    connectionId: string;
    calendars: CalendarListItem[];
  }>;
}): Promise<void> {
  const rows = await liveQueryClient.execute<CalendarSqlRow>(
    `
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
      WHERE provider = ?
      ORDER BY created_at, id
    `,
    [provider],
  );
  const existing = rows.map(normalizeCalendar);
  const existingByTrackingKey = new Map<string, StoredCalendar>();
  for (const calendar of existing) {
    const key = calendarKey(calendar);
    const current = existingByTrackingKey.get(key);
    if (!current || (current.deleted_at && !calendar.deleted_at)) {
      existingByTrackingKey.set(key, calendar);
    }
  }

  const requested = new Set(requestedConnectionIds);
  const successful = new Set(
    successfulConnections.map(({ connectionId }) => connectionId),
  );
  const incomingKeys = new Set(
    successfulConnections.flatMap(({ connectionId, calendars }) =>
      calendars.map((calendar) =>
        getCalendarTrackingKey({
          provider,
          connectionId,
          trackingId: calendar.id,
        }),
      ),
    ),
  );
  const now = new Date().toISOString();
  const statements: Statement[] = [];
  const calendarIdsToClear = new Set<string>();

  for (const calendar of existing) {
    if (calendar.deleted_at) continue;

    const disconnected = !requested.has(calendar.connection_id);
    const missingFromSuccessfulRefresh =
      successful.has(calendar.connection_id) &&
      !incomingKeys.has(calendarKey(calendar));

    if (disconnected || missingFromSuccessfulRefresh) {
      statements.push({
        sql: `
          UPDATE calendars
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, calendar.id],
      });
      calendarIdsToClear.add(calendar.id);
    } else if (!calendar.enabled) {
      calendarIdsToClear.add(calendar.id);
    }
  }

  if (calendarIdsToClear.size > 0) {
    const calendarIds = Array.from(calendarIdsToClear);
    statements.push({
      sql: `
        UPDATE events
        SET deleted_at = ?, updated_at = ?
        WHERE deleted_at IS NULL
          AND calendar_id IN (${placeholders(calendarIds.length)})
      `,
      params: [now, now, ...calendarIds],
    });
  }

  const seenIncomingKeys = new Set<string>();
  for (const { connectionId, calendars } of successfulConnections) {
    for (const calendar of calendars) {
      const key = getCalendarTrackingKey({
        provider,
        connectionId,
        trackingId: calendar.id,
      });
      if (seenIncomingKeys.has(key)) continue;
      seenIncomingKeys.add(key);

      const stored = existingByTrackingKey.get(key);
      const calendarId = stored?.id ?? id();
      statements.push({
        sql: `
          INSERT INTO calendars (
            id,
            tracking_id_calendar,
            name,
            enabled,
            provider,
            source,
            color,
            connection_id,
            created_at,
            updated_at,
            deleted_at
          )
          VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?, NULL)
          ON CONFLICT(id) DO UPDATE SET
            tracking_id_calendar = excluded.tracking_id_calendar,
            name = excluded.name,
            enabled = CASE
              WHEN calendars.deleted_at IS NULL THEN calendars.enabled
              ELSE 0
            END,
            provider = excluded.provider,
            source = excluded.source,
            color = excluded.color,
            connection_id = excluded.connection_id,
            updated_at = excluded.updated_at,
            deleted_at = NULL
        `,
        params: [
          calendarId,
          calendar.id,
          calendar.title,
          provider,
          calendar.source ?? "",
          calendar.color ?? "#888",
          connectionId,
          stored?.created_at ?? now,
          now,
        ],
      });
    }
  }

  if (statements.length > 0) {
    await executeTransaction(statements);
  }
}

export async function loadEventsForSync(
  ctx: Ctx,
  incomingTrackingIds: Iterable<string>,
): Promise<ExistingEvent[]> {
  const calendarIds = Array.from(ctx.calendarIds);
  if (calendarIds.length === 0) return [];

  const trackingIds = Array.from(new Set(incomingTrackingIds));
  const incomingClause =
    trackingIds.length > 0
      ? `OR tracking_id_event IN (${placeholders(trackingIds.length)})`
      : "";
  const rows = await liveQueryClient.execute<EventSqlRow>(
    `
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
        created_at,
        deleted_at
      FROM events
      WHERE calendar_id IN (${placeholders(calendarIds.length)})
        AND (
          (
            deleted_at IS NULL
            AND julianday(started_at) <= julianday(?)
            AND julianday(CASE WHEN ended_at = '' THEN started_at ELSE ended_at END)
              >= julianday(?)
          )
          ${incomingClause}
        )
      ORDER BY deleted_at IS NOT NULL, created_at, id
    `,
    [
      ...calendarIds,
      ctx.to.toISOString(),
      ctx.from.toISOString(),
      ...trackingIds,
    ],
  );

  return rows.map((row) => ({
    ...row,
    has_recurrence_rules: Boolean(row.has_recurrence_rules),
    is_all_day: Boolean(row.is_all_day),
  }));
}

export async function loadSessionsForTrackingIds(
  trackingIds: Iterable<string>,
): Promise<SessionSyncRow[]> {
  const ids = Array.from(new Set(trackingIds));
  if (ids.length === 0) return [];

  const rows = await liveQueryClient.execute<SessionSqlRow>(
    `
      SELECT id, owner_user_id, event_json, tracking_id
      FROM (
        SELECT
          session.id,
          session.owner_user_id,
          session.event_json,
          session.created_at,
          COALESCE(
            CASE
              WHEN json_valid(session.event_json)
              THEN NULLIF(
                CAST(json_extract(session.event_json, '$.tracking_id') AS TEXT),
                ''
              )
              ELSE NULL
            END,
            NULLIF(session.external_event_id, ''),
            NULLIF(event.tracking_id_event, '')
          ) AS tracking_id
        FROM sessions AS session
        LEFT JOIN events AS event
          ON event.id = session.event_id AND event.deleted_at IS NULL
        WHERE session.deleted_at IS NULL
      ) AS session_with_event
      WHERE tracking_id IN (${placeholders(ids.length)})
      ORDER BY created_at, id
    `,
    ids,
  );

  return rows.map((row) => ({
    id: row.id,
    ownerUserId: row.owner_user_id,
    eventJson: row.event_json,
    trackingId: row.tracking_id,
  }));
}

export async function loadParticipantSyncSnapshot(
  sessions: SessionSyncRow[],
  incomingParticipants: IncomingParticipants,
): Promise<ParticipantSyncSnapshot> {
  const sessionIds = sessions.map((session) => session.id);
  const emails = Array.from(
    new Set(
      Array.from(incomingParticipants.values())
        .flat()
        .map((participant) => participant.email?.trim().toLowerCase())
        .filter((email): email is string => Boolean(email)),
    ),
  );

  const [humanRows, mappingRows] = await Promise.all([
    emails.length > 0
      ? liveQueryClient.execute<HumanSqlRow>(
          `
            SELECT id, email
            FROM humans
            WHERE deleted_at IS NULL
              AND lower(email) IN (${placeholders(emails.length)})
            ORDER BY created_at, id
          `,
          emails,
        )
      : Promise.resolve([]),
    sessionIds.length > 0
      ? liveQueryClient.execute<ParticipantMappingSqlRow>(
          `
            SELECT id, session_id, human_id, source
            FROM session_participants
            WHERE deleted_at IS NULL
              AND session_id IN (${placeholders(sessionIds.length)})
            ORDER BY created_at, id
          `,
          sessionIds,
        )
      : Promise.resolve([]),
  ]);

  return {
    sessions,
    humans: humanRows,
    mappings: mappingRows.map((row) => ({
      id: row.id,
      sessionId: row.session_id,
      humanId: row.human_id,
      source: row.source,
    })),
  };
}

export async function applyConnectionSync({
  ctx,
  events,
  sessionUpdates,
  participants,
}: {
  ctx: Ctx;
  events: EventsSyncOutput;
  sessionUpdates: SessionEventUpdate[];
  participants: ParticipantsSyncOutput;
}): Promise<void> {
  const now = new Date().toISOString();
  const statements: Statement[] = [];
  const eventIdsByKey = new Map<string, string>();

  for (const eventId of events.toDelete) {
    statements.push({
      sql: `
        UPDATE events
        SET deleted_at = ?, updated_at = ?
        WHERE id = ? AND deleted_at IS NULL
      `,
      params: [now, now, eventId],
    });
  }

  for (const event of events.toUpdate) {
    eventIdsByKey.set(
      eventKey(event.calendar_id, event.tracking_id_event),
      event.id,
    );
    statements.push({
      sql: `
        UPDATE events
        SET
          tracking_id_event = ?,
          calendar_id = ?,
          title = ?,
          started_at = ?,
          ended_at = ?,
          location = ?,
          meeting_link = ?,
          description = ?,
          recurrence_series_id = ?,
          has_recurrence_rules = ?,
          is_all_day = ?,
          provider = ?,
          participants_json = ?,
          updated_at = ?,
          deleted_at = NULL
        WHERE id = ?
      `,
      params: [
        event.tracking_id_event,
        event.calendar_id,
        event.title,
        event.started_at,
        event.ended_at,
        event.location ?? "",
        event.meeting_link ?? "",
        event.description ?? "",
        event.recurrence_series_id ?? "",
        Number(event.has_recurrence_rules),
        Number(event.is_all_day),
        ctx.provider,
        encodeParticipants(event.participants),
        now,
        event.id,
      ],
    });
  }

  for (const event of events.toAdd) {
    const calendarId = ctx.calendarTrackingIdToId.get(
      event.tracking_id_calendar,
    );
    if (!calendarId) continue;

    const eventId = id();
    eventIdsByKey.set(eventKey(calendarId, event.tracking_id_event), eventId);
    statements.push({
      sql: `
        INSERT INTO events (
          id,
          tracking_id_event,
          calendar_id,
          title,
          started_at,
          ended_at,
          location,
          meeting_link,
          description,
          recurrence_series_id,
          has_recurrence_rules,
          is_all_day,
          provider,
          participants_json,
          created_at,
          updated_at,
          deleted_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
      `,
      params: [
        eventId,
        event.tracking_id_event,
        calendarId,
        event.title ?? "",
        event.started_at ?? "",
        event.ended_at ?? "",
        event.location ?? "",
        event.meeting_link ?? "",
        event.description ?? "",
        event.recurrence_series_id ?? "",
        Number(event.has_recurrence_rules),
        Number(event.is_all_day),
        ctx.provider,
        encodeParticipants(event.participants),
        now,
        now,
      ],
    });
  }

  for (const update of sessionUpdates) {
    const eventId =
      eventIdsByKey.get(eventKey(update.calendarId, update.trackingId)) ?? "";
    statements.push({
      sql: `
        UPDATE sessions
        SET
          event_id = CASE WHEN ? = '' THEN event_id ELSE ? END,
          external_event_id = ?,
          external_provider = ?,
          series_id = ?,
          event_json = ?,
          updated_at = ?
        WHERE id = ? AND deleted_at IS NULL
      `,
      params: [
        eventId,
        eventId,
        update.trackingId,
        ctx.provider,
        update.seriesId,
        update.eventJson,
        now,
        update.sessionId,
      ],
    });
  }

  for (const human of participants.humansToCreate) {
    statements.push({
      sql: `
        INSERT INTO humans (
          id,
          workspace_id,
          owner_user_id,
          name,
          email,
          created_at,
          updated_at,
          deleted_at
        )
        SELECT
          ?,
          NULLIF((
            SELECT json_extract(value_json, '$.workspace_id')
            FROM app_settings
            WHERE id = 'cloudsync_workspace_binding'
          ), ''),
          COALESCE(
            NULLIF(NULLIF(?, ''), '${DEFAULT_USER_ID}'),
            NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), ''),
            '${DEFAULT_USER_ID}'
          ),
          ?,
          ?,
          ?,
          ?,
          NULL
        WHERE NOT EXISTS (
          SELECT 1
          FROM humans
          WHERE deleted_at IS NULL AND lower(email) = lower(?)
        )
      `,
      params: [
        human.id,
        human.ownerUserId,
        human.name,
        human.email,
        now,
        now,
        human.email,
      ],
    });
  }

  for (const mappingId of participants.toDelete) {
    statements.push({
      sql: `
        UPDATE session_participants
        SET deleted_at = ?, updated_at = ?
        WHERE id = ? AND source = 'auto' AND deleted_at IS NULL
      `,
      params: [now, now, mappingId],
    });
  }

  for (const mapping of participants.toAdd) {
    statements.push({
      sql: `
        INSERT INTO session_participants (
          id,
          workspace_id,
          owner_user_id,
          session_id,
          human_id,
          display_name,
          email,
          role,
          source,
          metadata_json,
          created_at,
          updated_at,
          deleted_at
        )
        SELECT
          ?,
          session.workspace_id,
          session.owner_user_id,
          session.id,
          human.id,
          human.name,
          human.email,
          '',
          'auto',
          '{}',
          ?,
          ?,
          NULL
        FROM sessions AS session
        JOIN humans AS human ON human.id = (
          SELECT candidate.id
          FROM humans AS candidate
          WHERE candidate.deleted_at IS NULL
            AND (candidate.id = ? OR lower(candidate.email) = lower(?))
          ORDER BY candidate.id <> ?, candidate.created_at, candidate.id
          LIMIT 1
        )
        WHERE session.id = ?
          AND session.deleted_at IS NULL
          AND NOT EXISTS (
            SELECT 1
            FROM session_participants AS existing
            WHERE existing.session_id = session.id
              AND existing.human_id = human.id
              AND existing.deleted_at IS NULL
          )
      `,
      params: [
        id(),
        now,
        now,
        mapping.humanId,
        mapping.email,
        mapping.humanId,
        mapping.sessionId,
      ],
    });
  }

  if (statements.length > 0) {
    await executeTransaction(statements);
  }
}

function normalizeCalendar(row: CalendarSqlRow): StoredCalendar {
  return { ...row, enabled: Boolean(row.enabled) };
}

function calendarKey(calendar: StoredCalendar): string {
  return getCalendarTrackingKey({
    provider: calendar.provider,
    connectionId: calendar.connection_id,
    trackingId: calendar.tracking_id_calendar,
  });
}

function eventKey(calendarId: string, trackingId: string): string {
  return `${calendarId}\u0000${trackingId}`;
}

function placeholders(count: number): string {
  return Array.from({ length: count }, () => "?").join(", ");
}

function encodeParticipants(value: unknown[]): string | null {
  return value.length > 0 ? JSON.stringify(value) : null;
}
