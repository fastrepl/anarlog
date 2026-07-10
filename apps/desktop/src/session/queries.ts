import { json2md } from "@hypr/editor/markdown";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { EventParticipant, SessionEvent } from "@hypr/store";

import { executeTransaction, liveQueryClient } from "~/db";
import { DEFAULT_USER_ID, id } from "~/shared/utils";
import type { DeletedSessionData } from "~/store/zustand/undo-delete";

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
type SessionDeleteSqlRow = { id: string; title: string };
type SessionEmptySqlRow = {
  title: string;
  event_json: string;
  note_body: string;
  note_body_format: string;
  transcript_count: number;
  enhanced_note_count: number;
  manual_participant_count: number;
  tag_count: number;
};

export async function createSession(
  title = "",
  userId = DEFAULT_USER_ID,
): Promise<string> {
  const sessionId = id();
  const participantId = id();
  const now = new Date().toISOString();

  await executeTransaction([
    {
      sql: `
        INSERT INTO sessions (
          id, owner_user_id, title, created_at, updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, ?, NULL)
      `,
      params: [sessionId, userId, title, now, now],
    },
    createEmptyNoteStatement(sessionId, userId, now),
    {
      sql: `
        INSERT INTO humans (id, owner_user_id, updated_at, deleted_at)
        VALUES (?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
          deleted_at = NULL,
          updated_at = excluded.updated_at
      `,
      params: [userId, userId, now],
    },
    {
      sql: `
        INSERT INTO session_participants (
          id, owner_user_id, session_id, human_id, source, created_at,
          updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, 'manual', ?, ?, NULL)
      `,
      params: [participantId, userId, sessionId, userId, now, now],
    },
  ]);

  await trackNoteCreated(false);
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
          id, owner_user_id, title, created_at, updated_at, started_at,
          ended_at, event_id, external_event_id, external_provider, series_id,
          event_json, deleted_at
        )
        SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL
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
    createEmptyNoteStatement(sessionId, userId, now, true),
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
            id, owner_user_id, name, email, created_at, updated_at, deleted_at
          )
          SELECT ?, ?, ?, ?, ?, ?, NULL
          WHERE EXISTS (
            SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL
          )
        `,
        params: [
          humanId,
          userId,
          participant.name || email,
          email,
          now,
          now,
          sessionId,
        ],
      });
    }

    statements.push({
      sql: `
        INSERT INTO session_participants (
          id, owner_user_id, session_id, human_id, display_name, email,
          source, created_at, updated_at, deleted_at
        )
        SELECT ?, ?, ?, ?, ?, ?, 'auto', ?, ?, NULL
        WHERE EXISTS (
          SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL
        )
          AND NOT EXISTS (
            SELECT 1
            FROM session_participants
            WHERE session_id = ? AND human_id = ? AND deleted_at IS NULL
          )
      `,
      params: [
        id(),
        userId,
        sessionId,
        humanId,
        participant.name || email,
        email,
        now,
        now,
        sessionId,
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
    await trackNoteCreated(true);
  }
  return createdSessionId;
}

export async function softDeleteSession(
  sessionId: string,
): Promise<DeletedSessionData | null> {
  const [session] = await liveQueryClient.execute<SessionDeleteSqlRow>(
    `SELECT id, title FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1`,
    [sessionId],
  );
  if (!session) return null;

  const tombstone = new Date().toISOString();
  const rowsAffected = await executeTransaction(
    buildSessionTombstoneStatements(sessionId, tombstone),
  );
  if (rowsAffected[rowsAffected.length - 1] !== 1) return null;

  return {
    session: { id: session.id, title: session.title },
    tombstone,
    deletedAt: Date.now(),
  };
}

export async function isSessionEmpty(sessionId: string): Promise<boolean> {
  const [row] = await liveQueryClient.execute<SessionEmptySqlRow>(
    `
      SELECT
        sessions.title,
        sessions.event_json,
        COALESCE(note.body, '') AS note_body,
        COALESCE(note.body_format, '') AS note_body_format,
        (
          SELECT COUNT(*)
          FROM transcripts
          WHERE session_id = sessions.id AND deleted_at IS NULL
        ) AS transcript_count,
        (
          SELECT COUNT(*)
          FROM session_documents
          WHERE session_id = sessions.id
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
        ) AS enhanced_note_count,
        (
          SELECT COUNT(*)
          FROM session_participants
          WHERE session_id = sessions.id
            AND source NOT IN ('auto', 'excluded')
            AND human_id <> sessions.owner_user_id
            AND deleted_at IS NULL
        ) AS manual_participant_count,
        (
          SELECT COUNT(*)
          FROM session_tags
          WHERE session_id = sessions.id AND deleted_at IS NULL
        ) AS tag_count
      FROM sessions
      LEFT JOIN session_documents AS note
        ON note.id = sessions.id
        AND note.kind = 'note'
        AND note.deleted_at IS NULL
      WHERE sessions.id = ? AND sessions.deleted_at IS NULL
      LIMIT 1
    `,
    [sessionId],
  );

  if (!row) return true;
  if (row.title.trim() && !row.event_json) return false;
  if (hasNoteContent(row.note_body, row.note_body_format)) return false;

  return (
    Number(row.transcript_count) === 0 &&
    Number(row.enhanced_note_count) === 0 &&
    Number(row.manual_participant_count) === 0 &&
    Number(row.tag_count) === 0
  );
}

export async function restoreDeletedSession(
  data: DeletedSessionData,
): Promise<void> {
  await executeTransaction(
    buildSessionTombstoneStatements(data.session.id, data.tombstone, true),
  );
}

export async function finalizeSessionDeletion(
  sessionId: string,
): Promise<void> {
  try {
    const result = await fsSyncCommands.deleteSessionFolder(sessionId);
    if (result.status !== "error") return;
    console.error("[delete-session] failed to delete session folder", {
      sessionId,
      error: result.error,
    });
  } catch (error) {
    console.error("[delete-session] failed to delete session folder", {
      sessionId,
      error,
    });
  }
}

export function buildSessionTombstoneStatements(
  sessionId: string,
  tombstone: string,
  restore = false,
) {
  const value = restore ? null : tombstone;
  const predicate = restore ? "deleted_at = ?" : "deleted_at IS NULL";
  const predicateParams = restore ? [tombstone] : [];
  const directTables = [
    "session_documents",
    "transcripts",
    "session_participants",
    "session_tags",
    "action_items",
    "session_attachments",
  ];

  const statements = directTables.map((table) => ({
    sql: `
      UPDATE ${table}
      SET deleted_at = ?, updated_at = ?
      WHERE session_id = ? AND ${predicate}
    `,
    params: [value, tombstone, sessionId, ...predicateParams],
  }));

  statements.push({
    sql: `
      UPDATE entity_mentions
      SET deleted_at = ?, updated_at = ?
      WHERE (
        (source_type = 'session' AND source_id = ?)
        OR (target_type = 'session' AND target_id = ?)
      ) AND ${predicate}
    `,
    params: [value, tombstone, sessionId, sessionId, ...predicateParams],
  });
  statements.push({
    sql: `
      UPDATE sessions
      SET deleted_at = ?, updated_at = ?
      WHERE id = ? AND ${predicate}
    `,
    params: [value, tombstone, sessionId, ...predicateParams],
  });

  return statements;
}

function createEmptyNoteStatement(
  sessionId: string,
  userId: string,
  now: string,
  onlyIfSessionExists = false,
) {
  return {
    sql: `
      INSERT INTO session_documents (
        id, session_id, kind, body_format, body, created_by, updated_by,
        created_at, updated_at, deleted_at
      )
      ${onlyIfSessionExists ? "SELECT ?, ?, 'note', 'prosemirror_json', '', ?, ?, ?, ?, NULL" : "VALUES (?, ?, 'note', 'prosemirror_json', '', ?, ?, ?, ?, NULL)"}
      ${onlyIfSessionExists ? "WHERE EXISTS (SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)" : ""}
    `,
    params: onlyIfSessionExists
      ? [sessionId, sessionId, userId, userId, now, now, sessionId]
      : [sessionId, sessionId, userId, userId, now, now],
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

function hasNoteContent(body: string, format: string): boolean {
  if (!body) return false;

  let markdown = body;
  if (format === "prosemirror_json") {
    try {
      markdown = json2md(JSON.parse(body));
    } catch {
      markdown = body;
    }
  }

  markdown = markdown.trim();
  return Boolean(markdown && markdown !== "&nbsp;");
}

async function trackNoteCreated(hasEventId: boolean): Promise<void> {
  try {
    await analyticsCommands.event({
      event: "note_created",
      has_event_id: hasEventId,
    });
  } catch (error) {
    console.error("[session] failed to record note creation analytics", error);
  }
}
