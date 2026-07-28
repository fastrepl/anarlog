import {
  docToPlainText,
  isPlainTextDoc,
  markdownWithTitle,
  plainTextToDoc,
  retitleBody,
  stripMarkdownTitle,
} from "@/data/note-doc";
import { execute, executeTransaction, useLiveQuery } from "@/db";
import { captureAnalytics } from "@/lib/analytics";
import { DEFAULT_USER_ID, id, nowIso } from "@/lib/ids";

const SESSION_INSERT_SQL = `
INSERT INTO sessions (
  id, workspace_id, owner_user_id, title, event_json, created_at,
  updated_at, deleted_at
) VALUES (
  ?, NULLIF((
    SELECT json_extract(value_json, '$.workspace_id')
    FROM app_settings WHERE id = 'cloudsync_workspace_binding'
  ), ''), COALESCE(
    NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),
    NULLIF((
      SELECT json_extract(value_json, '$.workspace_id')
      FROM app_settings WHERE id = 'cloudsync_workspace_binding'
    ), '')
  ), ?, ?, ?, ?, NULL
)
`;

const NOTE_INSERT_SQL = `
INSERT INTO session_documents (
  id, workspace_id, session_id, kind, body_format, body, created_by,
  updated_by, created_at, updated_at, deleted_at
)
SELECT ?, workspace_id, id, 'note', 'prosemirror_json', ?,
  owner_user_id, owner_user_id, ?, ?, NULL
FROM sessions
WHERE id = ? AND deleted_at IS NULL
`;

const SELF_HUMAN_UPSERT_SQL = `
INSERT INTO humans (id, workspace_id, owner_user_id, updated_at, deleted_at)
SELECT session.owner_user_id, session.workspace_id, session.owner_user_id, ?, NULL
FROM sessions AS session WHERE session.id = ? AND session.deleted_at IS NULL
ON CONFLICT(id) DO UPDATE SET deleted_at = NULL, updated_at = excluded.updated_at
`;

const OWNER_PARTICIPANT_INSERT_SQL = `
INSERT INTO session_participants (
  id, workspace_id, owner_user_id, session_id, human_id, source,
  created_at, updated_at, deleted_at
)
SELECT ?, session.workspace_id, session.owner_user_id, session.id,
  session.owner_user_id, 'manual', ?, ?, NULL
FROM sessions AS session WHERE session.id = ? AND session.deleted_at IS NULL
`;

export async function createSession(options?: {
  title?: string;
  createdAt?: string;
  entryPoint?: "new_note" | "start_listening" | "voice_memo_import";
  trackCreated?: boolean;
}): Promise<string> {
  const sessionId = id();
  const participantId = id();
  const now = nowIso();
  const createdAt = options?.createdAt ?? now;
  const title = options?.title ?? "";

  await executeTransaction([
    {
      sql: SESSION_INSERT_SQL,
      params: [sessionId, DEFAULT_USER_ID, title, "", createdAt, createdAt],
    },
    { sql: NOTE_INSERT_SQL, params: [sessionId, "", now, now, sessionId] },
    { sql: SELF_HUMAN_UPSERT_SQL, params: [now, sessionId] },
    {
      sql: OWNER_PARTICIPANT_INSERT_SQL,
      params: [participantId, now, now, sessionId],
    },
  ]);

  if (options?.trackCreated !== false) {
    captureAnalytics("note_created", {
      entry_point: options?.entryPoint ?? "new_note",
      has_initial_title: Boolean(options?.title),
    });
  }
  return sessionId;
}

export type SessionDetail = {
  id: string;
  title: string;
  createdAt: string;
  noteText: string;
  bodyFormat: "prosemirror_json" | "markdown";
  plainEditable: boolean;
};

const SESSION_DETAIL_SQL = `
SELECT
  sessions.id,
  sessions.title,
  sessions.created_at,
  COALESCE(note.body, '') AS raw_body,
  COALESCE(note.body_format, 'prosemirror_json') AS raw_body_format
FROM sessions
LEFT JOIN session_documents AS note
  ON note.id = sessions.id AND note.kind = 'note' AND note.deleted_at IS NULL
WHERE sessions.id = ? AND sessions.deleted_at IS NULL
`;

type SessionDetailRow = {
  id: string;
  title: string;
  created_at: string;
  raw_body: string;
  raw_body_format: string;
};

function mapSessionDetailRows(rows: SessionDetailRow[]): SessionDetail | null {
  const row = rows[0];
  if (!row) return null;
  const isMarkdown = row.raw_body_format === "markdown";
  const noteText = isMarkdown
    ? stripMarkdownTitle(row.raw_body).text
    : docToPlainText(row.raw_body).text;
  return {
    id: row.id,
    title: row.title,
    createdAt: row.created_at,
    noteText,
    bodyFormat: isMarkdown ? "markdown" : "prosemirror_json",
    plainEditable: isMarkdown || isPlainTextDoc(row.raw_body),
  };
}

export function useSessionDetail(sessionId: string): {
  data: SessionDetail | null;
  isLoading: boolean;
} {
  const { data, isLoading } = useLiveQuery<
    SessionDetailRow,
    SessionDetail | null
  >({
    sql: SESSION_DETAIL_SQL,
    params: [sessionId],
    mapRows: mapSessionDetailRows,
  });
  return { data: data ?? null, isLoading };
}

const NOTE_UPSERT_SQL = `
INSERT INTO session_documents (
  id, workspace_id, session_id, kind, body_format, body, created_by,
  updated_by, created_at, updated_at, deleted_at
)
SELECT ?, workspace_id, id, 'note', ?, ?,
  owner_user_id, owner_user_id, ?, ?, NULL
FROM sessions
WHERE id = ? AND deleted_at IS NULL
ON CONFLICT(id) DO UPDATE SET
  body_format = excluded.body_format,
  body = excluded.body,
  updated_by = excluded.updated_by,
  updated_at = excluded.updated_at,
  deleted_at = NULL
`;

const SESSION_TITLE_UPDATE_SQL =
  "UPDATE sessions SET title = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL";
const NOTE_EDIT_ANALYTICS_SESSION_MS = 30 * 60 * 1_000;
const lastTrackedNoteEditAt = new Map<string, number>();

function captureNoteEditOnce(
  sessionId: string,
  editType: "body" | "title",
  bodyFormat?: "prosemirror_json" | "markdown",
) {
  const key = `${sessionId}:${editType}`;
  const now = Date.now();
  const lastTrackedAt = lastTrackedNoteEditAt.get(key);
  if (
    lastTrackedAt !== undefined &&
    now - lastTrackedAt < NOTE_EDIT_ANALYTICS_SESSION_MS
  ) {
    return;
  }

  lastTrackedNoteEditAt.set(key, now);
  captureAnalytics("note_edited", {
    edit_type: editType,
    ...(bodyFormat ? { body_format: bodyFormat } : {}),
  });
}

export async function saveSessionNote(
  sessionId: string,
  input: {
    title: string;
    bodyText: string;
    bodyFormat: "prosemirror_json" | "markdown";
  },
): Promise<void> {
  const now = nowIso();
  const body =
    input.bodyFormat === "markdown"
      ? markdownWithTitle(input.title, input.bodyText)
      : plainTextToDoc(input.title, input.bodyText);

  await executeTransaction([
    {
      sql: SESSION_TITLE_UPDATE_SQL,
      params: [input.title, now, sessionId],
    },
    {
      sql: NOTE_UPSERT_SQL,
      params: [sessionId, input.bodyFormat, body, now, now, sessionId],
    },
  ]);
  captureNoteEditOnce(sessionId, "body", input.bodyFormat);
}

// Mirrors desktop buildSessionTombstoneStatements: one transaction, the same
// tombstone timestamp on the session and all child rows.
const TOMBSTONE_CHILD_TABLES = [
  "session_documents",
  "transcripts",
  "session_participants",
  "session_tags",
  "action_items",
  "session_attachments",
];

export async function deleteSession(sessionId: string): Promise<void> {
  const now = nowIso();
  await executeTransaction([
    ...TOMBSTONE_CHILD_TABLES.map((table) => ({
      sql: `UPDATE ${table} SET deleted_at = ?, updated_at = ? WHERE session_id = ? AND deleted_at IS NULL`,
      params: [now, now, sessionId],
    })),
    {
      sql: "UPDATE entity_mentions SET deleted_at = ?, updated_at = ? WHERE deleted_at IS NULL AND ((source_type = 'session' AND source_id = ?) OR (target_type = 'session' AND target_id = ?))",
      params: [now, now, sessionId, sessionId],
    },
    {
      sql: "UPDATE sessions SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
      params: [now, now, sessionId],
    },
  ]);
}

export async function saveSessionTitle(
  sessionId: string,
  title: string,
): Promise<void> {
  const now = nowIso();
  const rows = await execute<{ body: string; body_format: string }>(
    "SELECT body, body_format FROM session_documents WHERE id = ? AND kind = 'note' AND deleted_at IS NULL LIMIT 1",
    [sessionId],
  );
  const statements = [
    { sql: SESSION_TITLE_UPDATE_SQL, params: [title, now, sessionId] },
  ];
  const note = rows[0];
  if (note) {
    const patched = retitleBody(note.body, note.body_format, title);
    if (patched !== null) {
      statements.push({
        // The body was read outside this transaction, so the write is guarded on
        // it: a saveSessionNote that landed in between must keep its newer text
        // rather than be overwritten by a stale patch.
        sql: "UPDATE session_documents SET body = ?, updated_at = ? WHERE id = ? AND kind = 'note' AND deleted_at IS NULL AND body = ?",
        params: [patched, now, sessionId, note.body],
      });
    }
  }
  await executeTransaction(statements);
  captureNoteEditOnce(sessionId, "title");
}
