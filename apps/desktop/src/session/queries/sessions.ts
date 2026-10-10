import { useCallback, useRef } from "react";

import { md2json } from "@anlg/editor/markdown";
import type { SessionEvent } from "@anlg/store";

import { extractBodyHeadingsKey } from "../title-content";
import type {
  SessionChanges,
  SessionRecord,
  SessionSummaryRecord,
} from "./types";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { isLockedFlag } from "~/lock/flag";

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
  locked: boolean | number;
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
const SESSION_PREFETCH_TTL_MS = 5_000;
const MAX_PREFETCHED_SESSIONS = 1;

type PrefetchedSession = {
  createdAt: number;
  promise: Promise<SessionRecord | null>;
  value?: SessionRecord | null;
};

const prefetchedSessions = new Map<string, PrefetchedSession>();

const SESSION_SELECT_SQL = `
  SELECT
    sessions.id,
    sessions.owner_user_id,
    sessions.created_at,
    sessions.folder_path,
    sessions.event_json,
    sessions.title,
    sessions.locked,
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
  const { data } = useLiveQuery<SessionSqlRow, SessionRecord | null>({
    sql: SESSION_SELECT_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => {
      const row = rows[0];
      return row ? mapSessionRow(row) : null;
    },
  });
  if (!sessionId) return null;
  if (data !== undefined) {
    prefetchedSessions.delete(sessionId);
    return data;
  }

  const prefetched = getPrefetchedSession(sessionId);
  return prefetched && "value" in prefetched
    ? (prefetched.value ?? null)
    : null;
}

export function preloadSession(
  sessionId: string,
): Promise<SessionRecord | null> {
  if (!sessionId) return Promise.resolve(null);

  const existing = getPrefetchedSession(sessionId);
  if (existing) return existing.promise;

  let entry: PrefetchedSession;
  const promise = liveQueryClient
    .execute<SessionSqlRow>(SESSION_SELECT_SQL, [sessionId])
    .then((rows) => {
      const row = rows[0];
      const value = row ? mapSessionRow(row) : null;
      if (prefetchedSessions.get(sessionId) === entry) {
        entry.value = value;
      }
      return value;
    })
    .catch((error) => {
      if (prefetchedSessions.get(sessionId) === entry) {
        prefetchedSessions.delete(sessionId);
      }
      throw error;
    });

  entry = { createdAt: Date.now(), promise };
  prefetchedSessions.set(sessionId, entry);
  trimPrefetchedSessions();
  return promise;
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

export function useSessionSummariesByIds(
  sessionIds: readonly string[],
): SessionSummaryRecord[] {
  const uniqueIds = [...new Set(sessionIds.filter(Boolean))].sort();
  const placeholders = uniqueIds.map(() => "?").join(", ");
  const enabled = uniqueIds.length > 0;
  const { data } = useLiveQuery<SessionSummarySqlRow, SessionSummaryRecord[]>({
    sql: `
      SELECT id, title, created_at
      FROM sessions
      WHERE id IN (${placeholders || "NULL"})
        AND deleted_at IS NULL
      ORDER BY id
    `,
    params: uniqueIds,
    enabled,
  });
  return useHeldLiveQueryRows(data, EMPTY_SESSION_SUMMARIES, enabled);
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

function parseJsonRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "string" || !value) {
    return {};
  }
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed !== null &&
      typeof parsed === "object" &&
      !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

// A memo without a stored key predates heading tracking. Compare against the
// stored body: unchanged headings keep the note's previous write time so a
// newer template keeps authority, while a real heading edit stamps now. An
// unreadable or missing baseline stamps now, favoring the memo.
function previousHeadingsTime(
  row: { updated_at?: string; body?: string; body_format?: string } | undefined,
  headingsKey: string,
  now: string,
): string {
  if (row?.body === undefined) {
    return now;
  }
  const storedKey = extractBodyHeadingsKey(row.body, row.body_format);
  if (storedKey === null || storedKey !== headingsKey) {
    return now;
  }
  return row.updated_at ?? now;
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
      [
        "locked",
        changes.locked === undefined ? undefined : Number(changes.locked),
      ],
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

    let noteMetadata: string | undefined;
    if (changes.raw_md !== undefined) {
      const existing =
        (await liveQueryClient.execute<{
          generation_metadata_json: string;
          updated_at: string;
          body: string;
          body_format: string;
        }>(
          `SELECT generation_metadata_json, updated_at, body, body_format FROM session_documents WHERE id = ? AND kind = 'note'`,
          [sessionId],
        )) ?? [];
      const metadata = parseJsonRecord(existing[0]?.generation_metadata_json);
      let changed = false;
      if (changes.raw_template_snapshot !== undefined) {
        metadata.appliedTemplate = changes.raw_template_snapshot;
        changed = true;
      }
      const headingsKey = extractBodyHeadingsKey(changes.raw_md);
      if (headingsKey !== null) {
        const stored = metadata.headings as
          | { key?: unknown; updatedAt?: unknown }
          | undefined;
        if (stored?.key !== headingsKey) {
          metadata.headings = {
            key: headingsKey,
            updatedAt:
              stored?.key === undefined
                ? previousHeadingsTime(existing[0], headingsKey, now)
                : now,
          };
          changed = true;
        }
      }
      if (changed) {
        noteMetadata = JSON.stringify(metadata);
      }
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

    if (noteMetadata !== undefined) {
      statements.push({
        sql: `UPDATE session_documents SET generation_metadata_json = ? WHERE id = ? AND kind = 'note'`,
        params: [noteMetadata, sessionId],
      });
    }

    if (statements.length > 0) await executeTransaction(statements);
  });
}

function useHeldLiveQueryRows<T>(
  data: T[] | undefined,
  empty: T[],
  enabled: boolean,
): T[] {
  const previous = useRef(empty);
  if (data !== undefined) {
    previous.current = data;
  }
  if (!enabled) {
    previous.current = empty;
    return empty;
  }
  return data ?? previous.current;
}

function getPrefetchedSession(sessionId: string) {
  const entry = prefetchedSessions.get(sessionId);
  if (!entry) return undefined;
  if (Date.now() - entry.createdAt <= SESSION_PREFETCH_TTL_MS) return entry;
  prefetchedSessions.delete(sessionId);
  return undefined;
}

function trimPrefetchedSessions() {
  while (prefetchedSessions.size > MAX_PREFETCHED_SESSIONS) {
    const oldestSessionId = prefetchedSessions.keys().next().value;
    if (!oldestSessionId) return;
    prefetchedSessions.delete(oldestSessionId);
  }
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
    locked: isLockedFlag(row.locked),
  };
}
