import { useCallback } from "react";

import { md2json } from "@anlg/editor/markdown";
import type { SessionEvent } from "@anlg/store";

import type {
  SessionChanges,
  SessionRecord,
  SessionSummaryRecord,
} from "./types";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

type SessionSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  folder_path: string;
  event_json: string;
  title: string;
  raw_body: string;
  raw_body_format: string;
  raw_template_id: string;
};

type SessionSummarySqlRow = {
  id: string;
  title: string;
  created_at: string;
};

type SessionTranscriptStateSqlRow = {
  has_transcript: boolean | number;
};

type SessionEventSqlRow = { event_json: string };

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
    COALESCE(note.body_format, 'prosemirror_json') AS raw_body_format,
    COALESCE(note.template_id, '') AS raw_template_id
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

export function useSessionTranscriptExistence(
  sessionId: string,
): boolean | null {
  const { data = null } = useLiveQuery<
    SessionTranscriptStateSqlRow,
    boolean | null
  >({
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

export function useSessionHasTranscript(sessionId: string): boolean {
  return useSessionTranscriptExistence(sessionId) === true;
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
      const hasTemplateChange = changes.raw_template_id !== undefined;
      statements.push({
        sql: `
          INSERT INTO session_documents (
            id, workspace_id, session_id, kind, template_id, body_format, body,
            created_by, updated_by, created_at, updated_at, deleted_at
          )
          SELECT ?, workspace_id, id, 'note', ?, 'prosemirror_json', ?,
            owner_user_id, owner_user_id, ?, ?, NULL
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
          ON CONFLICT(id) DO UPDATE SET
            ${hasTemplateChange ? "template_id = excluded.template_id," : ""}
            body_format = excluded.body_format,
            body = excluded.body,
            updated_by = excluded.updated_by,
            updated_at = excluded.updated_at,
            deleted_at = NULL
        `,
        params: [
          sessionId,
          changes.raw_template_id ?? "",
          changes.raw_md,
          now,
          now,
          sessionId,
        ],
      });
    }

    if (statements.length > 0) await executeTransaction(statements);
  });
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
    raw_template_id: row.raw_template_id,
  };
}
