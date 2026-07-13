import { useCallback } from "react";

import { json2md, md2json } from "@hypr/editor/markdown";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { EventParticipant, SessionEvent } from "@hypr/store";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
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
type SessionEventSqlRow = { event_json: string };
type SessionDeleteSqlRow = { id: string; title: string };
type SessionEmptySqlRow = {
  title: string;
  event_json: string;
  note_body: string;
  note_body_format: string;
  transcript_count: number;
  enhanced_note_count: number;
  meeting_chat_count: number;
  manual_participant_count: number;
  tag_count: number;
};

type SessionSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  folder_path: string;
  event_json: string;
  title: string;
  raw_body: string;
  raw_body_format: string;
};

type SessionSummarySqlRow = {
  id: string;
  title: string;
  created_at: string;
};

type SessionTranscriptStateSqlRow = {
  has_transcript: boolean | number;
};

type SessionParticipantSqlRow = {
  id: string;
  session_id: string;
  human_id: string;
  source: string;
  name: string;
  email: string;
  job_title: string;
  linkedin_username: string;
  organization_id: string;
  organization_name: string;
};

type EnhancedNoteSqlRow = {
  id: string;
  session_id: string;
  title: string;
  body: string;
  body_format: string;
  template_id: string;
  sort_order: number;
};

export type SessionRecord = {
  id: string;
  user_id: string;
  created_at: string;
  folder_id: string;
  event_json: string;
  title: string;
  raw_md: string;
};

export type SessionChanges = Partial<
  Pick<
    SessionRecord,
    "created_at" | "event_json" | "folder_id" | "raw_md" | "title"
  >
>;

export type SessionSummaryRecord = {
  id: string;
  title: string;
  created_at: string;
};

export type EnhancedNoteRecord = {
  id: string;
  sessionId: string;
  title: string;
  content: string;
  templateId: string;
  position: number;
};

export type SessionParticipantRecord = {
  id: string;
  sessionId: string;
  humanId: string;
  source: string;
  name: string;
  email: string;
  jobTitle: string;
  linkedinUsername: string;
  organizationId: string;
  organizationName: string;
};

const EMPTY_ENHANCED_NOTES: EnhancedNoteRecord[] = [];
const EMPTY_SESSION_PARTICIPANTS: SessionParticipantRecord[] = [];
const EMPTY_SESSION_SUMMARIES: SessionSummaryRecord[] = [];

const SESSION_SELECT_SQL = `
  SELECT
    sessions.id,
    sessions.owner_user_id,
    sessions.created_at,
    sessions.folder_path,
    sessions.event_json,
    sessions.title,
    COALESCE(note.body, '') AS raw_body,
    COALESCE(note.body_format, 'prosemirror_json') AS raw_body_format
  FROM sessions
  LEFT JOIN session_documents AS note
    ON note.id = sessions.id
    AND note.kind = 'note'
    AND note.deleted_at IS NULL
  WHERE sessions.id = ? AND sessions.deleted_at IS NULL
  LIMIT 1
`;

export function useSession(sessionId: string): SessionRecord | null {
  const { data = null } = useLiveQuery<SessionSqlRow, SessionRecord | null>({
    sql: SESSION_SELECT_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => {
      const row = rows[0];
      return row ? mapSessionRow(row) : null;
    },
  });
  return sessionId ? data : null;
}

export function useSessionSummary(
  sessionId: string,
): SessionSummaryRecord | null {
  const { data = null } = useLiveQuery<
    SessionSummarySqlRow,
    SessionSummaryRecord | null
  >({
    sql: `
      SELECT id, title, created_at
      FROM sessions
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows[0] ?? null,
  });
  return sessionId ? data : null;
}

export function useSessionSummaries(): SessionSummaryRecord[] {
  const { data = EMPTY_SESSION_SUMMARIES } = useLiveQuery<
    SessionSummarySqlRow,
    SessionSummaryRecord[]
  >({
    sql: `
      SELECT id, title, created_at
      FROM sessions
      WHERE deleted_at IS NULL
      ORDER BY created_at DESC, id
    `,
  });
  return data;
}

export async function loadSessionEvent(
  sessionId: string,
): Promise<SessionEvent | null> {
  const rows = await liveQueryClient.execute<SessionEventSqlRow>(
    `
      SELECT event_json
      FROM sessions
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [sessionId],
  );
  const eventJson = rows[0]?.event_json;
  if (!eventJson) return null;

  try {
    return JSON.parse(eventJson) as SessionEvent;
  } catch {
    return null;
  }
}

export function useUpdateSession(sessionId: string) {
  return useCallback(
    (changes: SessionChanges) => updateSession(sessionId, changes),
    [sessionId],
  );
}

export function useSessionHasTranscript(sessionId: string): boolean {
  const { data = false } = useLiveQuery<SessionTranscriptStateSqlRow, boolean>({
    sql: `
      SELECT EXISTS (
        SELECT 1
        FROM transcripts
        WHERE session_id = ?
          AND deleted_at IS NULL
          AND CASE
            WHEN json_valid(words_json) THEN json_array_length(words_json)
            ELSE 0
          END > 0
      ) AS has_transcript
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => Boolean(rows[0]?.has_transcript),
  });
  return sessionId ? data : false;
}

export function useSessionParticipants(
  sessionId: string,
): SessionParticipantRecord[] {
  const { data = EMPTY_SESSION_PARTICIPANTS } = useLiveQuery<
    SessionParticipantSqlRow,
    SessionParticipantRecord[]
  >({
    sql: `
      SELECT
        participant.id,
        participant.session_id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email,
        COALESCE(human.job_title, '') AS job_title,
        COALESCE(human.linkedin_username, '') AS linkedin_username,
        COALESCE(human.organization_id, '') AS organization_id,
        COALESCE(organization.name, '') AS organization_name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      LEFT JOIN organizations AS organization
        ON organization.id = human.organization_id
        AND organization.deleted_at IS NULL
      WHERE participant.session_id = ?
        AND participant.deleted_at IS NULL
      ORDER BY name, email, participant.id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapSessionParticipantRow),
  });
  return sessionId ? data : EMPTY_SESSION_PARTICIPANTS;
}

export function useSessionParticipant(
  mappingId: string,
): SessionParticipantRecord | null {
  const { data = null } = useLiveQuery<
    SessionParticipantSqlRow,
    SessionParticipantRecord | null
  >({
    sql: `
      SELECT
        participant.id,
        participant.session_id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email,
        COALESCE(human.job_title, '') AS job_title,
        COALESCE(human.linkedin_username, '') AS linkedin_username,
        COALESCE(human.organization_id, '') AS organization_id,
        COALESCE(organization.name, '') AS organization_name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      LEFT JOIN organizations AS organization
        ON organization.id = human.organization_id
        AND organization.deleted_at IS NULL
      WHERE participant.id = ? AND participant.deleted_at IS NULL
      LIMIT 1
    `,
    params: [mappingId],
    enabled: Boolean(mappingId),
    mapRows: (rows) => (rows[0] ? mapSessionParticipantRow(rows[0]) : null),
  });
  return mappingId ? data : null;
}

export function addSessionParticipant(
  sessionId: string,
  humanId: string,
  source = "manual",
): Promise<void> {
  return enqueueDatabaseWrite("session-participants", async () => {
    const participantId = id();
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants
          SET source = ?, updated_at = ?
          WHERE id = (
            SELECT id
            FROM session_participants
            WHERE session_id = ?
              AND human_id = ?
              AND source = 'excluded'
              AND deleted_at IS NULL
              AND ? <> 'auto'
            ORDER BY created_at, id
            LIMIT 1
          )
        `,
        params: [source, now, sessionId, humanId, source],
      },
      {
        sql: `
          INSERT INTO session_participants (
            id, workspace_id, owner_user_id, session_id, human_id,
            display_name, email, role, source, metadata_json, created_at,
            updated_at, deleted_at
          )
          SELECT ?, session.workspace_id, session.owner_user_id, session.id, human.id,
            human.name, human.email, '', ?, '{}', ?, ?, NULL
          FROM sessions AS session
          JOIN humans AS human ON human.id = ? AND human.deleted_at IS NULL
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
        params: [participantId, source, now, now, humanId, sessionId],
      },
    ]);
  });
}

export function removeSessionParticipant(mappingId: string): Promise<void> {
  return enqueueDatabaseWrite("session-participants", async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants
          SET
            source = CASE WHEN source = 'auto' THEN 'excluded' ELSE source END,
            deleted_at = CASE WHEN source = 'auto' THEN NULL ELSE ? END,
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, mappingId],
      },
    ]);
  });
}

export function useEnhancedNoteRecords(
  sessionId: string,
): EnhancedNoteRecord[] {
  const { data = EMPTY_ENHANCED_NOTES } = useLiveQuery<
    EnhancedNoteSqlRow,
    EnhancedNoteRecord[]
  >({
    sql: `
      SELECT
        id,
        session_id,
        title,
        body,
        body_format,
        template_id,
        sort_order
      FROM session_documents
      WHERE session_id = ?
        AND kind IN ('summary', 'template_output')
        AND deleted_at IS NULL
      ORDER BY sort_order, id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapEnhancedNoteRow),
  });
  return sessionId ? data : EMPTY_ENHANCED_NOTES;
}

export function useEnhancedNote(
  enhancedNoteId: string,
): EnhancedNoteRecord | null {
  const { data = null } = useLiveQuery<
    EnhancedNoteSqlRow,
    EnhancedNoteRecord | null
  >({
    sql: `
      SELECT
        id,
        session_id,
        title,
        body,
        body_format,
        template_id,
        sort_order
      FROM session_documents
      WHERE id = ?
        AND kind IN ('summary', 'template_output')
        AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [enhancedNoteId],
    enabled: Boolean(enhancedNoteId),
    mapRows: (rows) => {
      const row = rows[0];
      return row ? mapEnhancedNoteRow(row) : null;
    },
  });
  return enhancedNoteId ? data : null;
}

export function useUpdateEnhancedNoteContent(
  enhancedNoteId: string,
  sessionId: string,
) {
  return useCallback(
    (content: string, sessionTitle?: string) =>
      updateEnhancedNoteContent(
        enhancedNoteId,
        sessionId,
        content,
        sessionTitle,
      ),
    [enhancedNoteId, sessionId],
  );
}

export function updateEnhancedNoteContent(
  enhancedNoteId: string,
  sessionId: string,
  content: string,
  sessionTitle?: string,
): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const statements: Array<{ sql: string; params: unknown[] }> = [
      {
        sql: `
          UPDATE session_documents
          SET body = ?, body_format = 'prosemirror_json', updated_at = ?
          WHERE id = ?
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
        `,
        params: [content, now, enhancedNoteId],
      },
    ];

    if (sessionTitle !== undefined) {
      statements.push({
        sql: `
          UPDATE sessions
          SET title = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [sessionTitle, now, sessionId],
      });
    }

    await executeTransaction(statements);
  });
}

export function deleteEnhancedNote(enhancedNoteId: string): Promise<void> {
  return enqueueDatabaseWrite(`enhanced-note:${enhancedNoteId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_documents
          SET deleted_at = ?, updated_at = ?
          WHERE id = ?
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
        `,
        params: [now, now, enhancedNoteId],
      },
    ]);
  });
}

export function updateSession(
  sessionId: string,
  changes: SessionChanges,
): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const assignments: string[] = [];
    const params: unknown[] = [];

    for (const [column, value] of [
      ["title", changes.title],
      ["created_at", changes.created_at],
      ["folder_path", changes.folder_id],
      ["event_json", changes.event_json],
    ] as const) {
      if (value === undefined) continue;
      assignments.push(`${column} = ?`);
      params.push(value);
    }

    const statements: Array<{ sql: string; params: unknown[] }> = [];
    if (assignments.length > 0) {
      statements.push({
        sql: `
          UPDATE sessions
          SET ${assignments.join(", ")}, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [...params, now, sessionId],
      });
    }

    if (changes.raw_md !== undefined) {
      statements.push({
        sql: `
          INSERT INTO session_documents (
            id, workspace_id, session_id, kind, body_format, body, created_by,
            updated_by, created_at, updated_at, deleted_at
          )
          SELECT ?, workspace_id, id, 'note', 'prosemirror_json', ?,
            owner_user_id, owner_user_id, ?, ?, NULL
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
          ON CONFLICT(id) DO UPDATE SET
            body_format = excluded.body_format,
            body = excluded.body,
            updated_by = excluded.updated_by,
            updated_at = excluded.updated_at,
            deleted_at = NULL
        `,
        params: [sessionId, changes.raw_md, now, now, sessionId],
      });
    }

    if (statements.length > 0) await executeTransaction(statements);
  });
}

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
          FROM session_documents
          WHERE session_id = sessions.id
            AND kind = 'meeting_chat'
            AND deleted_at IS NULL
        ) AS meeting_chat_count,
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
    Number(row.meeting_chat_count) === 0 &&
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

function mapSessionRow(row: SessionSqlRow): SessionRecord {
  let rawMd = row.raw_body;
  if (rawMd && row.raw_body_format === "markdown") {
    try {
      rawMd = JSON.stringify(md2json(rawMd));
    } catch (error) {
      console.error("[session] failed to decode imported Markdown", error);
    }
  }

  return {
    id: row.id,
    user_id: row.owner_user_id,
    created_at: row.created_at,
    folder_id: row.folder_path,
    event_json: row.event_json,
    title: row.title,
    raw_md: rawMd,
  };
}

function mapSessionParticipantRow(
  row: SessionParticipantSqlRow,
): SessionParticipantRecord {
  return {
    id: row.id,
    sessionId: row.session_id,
    humanId: row.human_id,
    source: row.source,
    name: row.name,
    email: row.email,
    jobTitle: row.job_title,
    linkedinUsername: row.linkedin_username,
    organizationId: row.organization_id,
    organizationName: row.organization_name,
  };
}

function mapEnhancedNoteRow(row: EnhancedNoteSqlRow): EnhancedNoteRecord {
  let content = row.body;
  if (content && row.body_format === "markdown") {
    try {
      content = JSON.stringify(md2json(content));
    } catch (error) {
      console.error("[session] failed to decode summary Markdown", error);
    }
  }

  return {
    id: row.id,
    sessionId: row.session_id,
    title: row.title,
    content,
    templateId: row.template_id,
    position: Number(row.sort_order),
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
