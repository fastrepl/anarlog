import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import type { EventParticipant, SessionEvent } from "@anlg/store";

import type { SessionChanges } from "./types";

import { executeTransaction, liveQueryClient } from "~/db";
import { DEFAULT_USER_ID, id } from "~/shared/utils";

type EventSqlRow = {
  id: string;
  tracking_id_event: string;
  calendar_id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location: string;
  meeting_link: string;
  description: string;
  recurrence_series_id: string;
  has_recurrence_rules: boolean | number;
  is_all_day: boolean | number;
  provider: string;
  participants_json: string | null;
};

type HumanEmailSqlRow = { id: string; email: string };
type SessionIdentitySqlRow = { id: string };

export async function createSession(
  title = "",
  userId = DEFAULT_USER_ID,
  initial?: Pick<SessionChanges, "event_json" | "raw_md">,
): Promise<string> {
  const sessionId = id();
  const participantId = id();
  const now = new Date().toISOString();

  await executeTransaction([
    {
      sql: `
        INSERT INTO sessions (
          id, workspace_id, owner_user_id, title, event_json, created_at,
          updated_at, deleted_at
        ) VALUES (
          ?, NULLIF((
            SELECT json_extract(value_json, '$.workspace_id')
            FROM app_settings
            WHERE id = 'cloudsync_workspace_binding'
          ), ''), COALESCE(
            NULLIF(NULLIF(?, ''), '${DEFAULT_USER_ID}'),
            NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), '')
          ), ?, ?, ?, ?, NULL
        )
      `,
      params: [sessionId, userId, title, initial?.event_json ?? "", now, now],
    },
    createEmptyNoteStatement(sessionId, now, initial?.raw_md ?? ""),
    {
      sql: `
        INSERT INTO humans (
          id, workspace_id, owner_user_id, updated_at, deleted_at
        )
        SELECT session.owner_user_id, session.workspace_id,
          session.owner_user_id, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
        ON CONFLICT(id) DO UPDATE SET
          deleted_at = NULL,
          updated_at = excluded.updated_at
      `,
      params: [now, sessionId],
    },
    {
      sql: `
        INSERT INTO session_participants (
          id, workspace_id, owner_user_id, session_id, human_id, source,
          created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, session.id,
          session.owner_user_id, 'manual', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
      `,
      params: [participantId, now, now, sessionId],
    },
  ]);

  trackNoteCreated(false);
  return sessionId;
}

export async function getOrCreateSessionForEventId(
  eventId: string,
  title?: string,
  userId = DEFAULT_USER_ID,
): Promise<string> {
  const [event] = await liveQueryClient.execute<EventSqlRow>(
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
        recurrence_series_id,
        has_recurrence_rules,
        is_all_day,
        provider,
        participants_json
      FROM events
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [eventId],
  );

  if (!event) {
    return createSession(title, userId);
  }

  const existingSessionId = await findSessionForEvent(event);
  if (existingSessionId) {
    return existingSessionId;
  }

  const sessionId = id();
  const now = new Date().toISOString();
  const sessionEvent = toSessionEvent(event);
  const participants = parseEventParticipants(event.participants_json);
  const humansByEmail = await findHumansByEmail(participants);
  const statements = [
    {
      sql: `
        INSERT INTO sessions (
          id, workspace_id, owner_user_id, title, created_at, updated_at,
          started_at, ended_at, event_id, external_event_id, external_provider,
          series_id, event_json, deleted_at
        )
        SELECT ?, NULLIF((
          SELECT json_extract(value_json, '$.workspace_id')
          FROM app_settings
          WHERE id = 'cloudsync_workspace_binding'
        ), ''), COALESCE(
          NULLIF(NULLIF(?, ''), '${DEFAULT_USER_ID}'),
          NULLIF((
            SELECT json_extract(value_json, '$.workspace_id')
            FROM app_settings
            WHERE id = 'cloudsync_workspace_binding'
          ), '')
        ), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL
        WHERE NOT EXISTS (
          SELECT 1
          FROM sessions
          WHERE deleted_at IS NULL
            AND (event_id = ? OR (? <> '' AND external_event_id = ?))
        )
      `,
      params: [
        sessionId,
        userId,
        title ?? sessionEvent.title,
        now,
        now,
        sessionEvent.started_at,
        sessionEvent.ended_at,
        event.id,
        event.tracking_id_event,
        event.provider,
        event.recurrence_series_id,
        JSON.stringify(sessionEvent),
        event.id,
        event.tracking_id_event,
        event.tracking_id_event,
      ],
    },
    createEmptyNoteStatement(sessionId, now),
  ];

  const seenEmails = new Set<string>();
  for (const participant of participants) {
    const email = participant.email?.trim();
    if (!email) continue;
    const emailKey = email.toLowerCase();
    if (seenEmails.has(emailKey)) continue;
    seenEmails.add(emailKey);

    const humanId = humansByEmail.get(emailKey) ?? id();
    if (!humansByEmail.has(emailKey)) {
      statements.push({
        sql: `
          INSERT INTO humans (
            id, workspace_id, owner_user_id, name, email, created_at,
            updated_at, deleted_at
          )
          SELECT ?, session.workspace_id, session.owner_user_id, ?, ?, ?, ?, NULL
          FROM sessions AS session
          WHERE session.id = ? AND session.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1
              FROM humans
              WHERE lower(email) = lower(?) AND deleted_at IS NULL
            )
        `,
        params: [
          humanId,
          participant.name || email,
          email,
          now,
          now,
          sessionId,
          email,
        ],
      });
    }

    statements.push({
      sql: `
        INSERT INTO session_participants (
          id, workspace_id, owner_user_id, session_id, human_id, display_name,
          email, source, created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, session.id,
          ?, ?, ?, 'auto', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
          AND NOT EXISTS (
            SELECT 1
            FROM session_participants
            WHERE session_id = session.id AND human_id = ? AND deleted_at IS NULL
          )
      `,
      params: [
        id(),
        humanId,
        participant.name || email,
        email,
        now,
        now,
        sessionId,
        humanId,
      ],
    });
  }

  const rowsAffected = await executeTransaction(statements);

  const createdSessionId = await findSessionForEvent(event, sessionId);
  if (!createdSessionId) {
    throw new Error(`Failed to create a session for event ${eventId}`);
  }

  if (rowsAffected[0] === 1) {
    trackNoteCreated(true);
  }
  return createdSessionId;
}

function createEmptyNoteStatement(sessionId: string, now: string, body = "") {
  return {
    sql: `
      INSERT INTO session_documents (
        id, workspace_id, session_id, kind, body_format, body, created_by,
        updated_by, created_at, updated_at, deleted_at
      )
      SELECT ?, workspace_id, id, 'note', 'prosemirror_json', ?,
        owner_user_id, owner_user_id, ?, ?, NULL
      FROM sessions
      WHERE id = ? AND deleted_at IS NULL
    `,
    params: [sessionId, body, now, now, sessionId],
  };
}

async function findSessionForEvent(
  event: EventSqlRow,
  preferredId?: string,
): Promise<string | null> {
  const rows = await liveQueryClient.execute<SessionIdentitySqlRow>(
    `
      SELECT id
      FROM sessions
      WHERE deleted_at IS NULL
        AND (event_id = ? OR (? <> '' AND external_event_id = ?))
      ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id
      LIMIT 1
    `,
    [
      event.id,
      event.tracking_id_event,
      event.tracking_id_event,
      preferredId ?? "",
    ],
  );
  return rows[0]?.id ?? null;
}

async function findHumansByEmail(
  participants: EventParticipant[],
): Promise<Map<string, string>> {
  const emails = Array.from(
    new Set(
      participants
        .map((participant) => participant.email?.trim().toLowerCase())
        .filter((email): email is string => Boolean(email)),
    ),
  );
  if (emails.length === 0) return new Map();

  const rows = await liveQueryClient.execute<HumanEmailSqlRow>(
    `
      SELECT id, email
      FROM humans
      WHERE deleted_at IS NULL
        AND lower(email) IN (${emails.map(() => "?").join(", ")})
      ORDER BY id
    `,
    emails,
  );
  return new Map(rows.map((row) => [row.email.toLowerCase(), row.id]));
}

function parseEventParticipants(value: string | null): EventParticipant[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed) ? (parsed as EventParticipant[]) : [];
  } catch {
    return [];
  }
}

function toSessionEvent(event: EventSqlRow): SessionEvent {
  return {
    tracking_id: event.tracking_id_event,
    calendar_id: event.calendar_id,
    title: event.title,
    started_at: event.started_at,
    ended_at: event.ended_at,
    is_all_day: Boolean(event.is_all_day),
    has_recurrence_rules: Boolean(event.has_recurrence_rules),
    location: event.location,
    meeting_link: event.meeting_link,
    description: event.description,
    recurrence_series_id: event.recurrence_series_id,
  };
}

function trackNoteCreated(hasEventId: boolean): void {
  void analyticsCommands
    .eventFireAndForget({
      event: "note_created",
      has_event_id: hasEventId,
    })
    .catch((error) => {
      console.error(
        "[session] failed to record note creation analytics",
        error,
      );
    });
}
