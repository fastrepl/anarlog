import { md2json } from "@anlg/editor/markdown";
import { commands } from "@anlg/plugin-session";

import { updateSession } from "./sessions";

import { useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

export type SessionConflictRecord = {
  id: string;
  field: "body" | "title";
  lostSide: string;
  editedAtMs: number | null;
  createdAt: string;
  value: string;
  bodyFormat: string;
};

export type SessionDocumentVersionRecord = {
  id: string;
  body: string;
  bodyFormat: string;
  source: string;
  createdAt: string;
};

type ConflictSqlRow = {
  id: string;
  field_name: string;
  lost_side: string;
  edited_at_ms: number | null;
  value_json: string;
  created_at: string;
  body_format: string;
};

type VersionSqlRow = {
  id: string;
  body: string;
  body_format: string;
  source: string;
  created_at: string;
};

const PREVIEW_LENGTH = 160;

const EMPTY_CONFLICTS: SessionConflictRecord[] = [];
const EMPTY_VERSIONS: SessionDocumentVersionRecord[] = [];

// Note documents use id == session_id, so both conflict kinds key off the id.
const NOTE_CONFLICT_FIELDS_SQL = `
  (table_name = 'session_documents' AND field_name = 'body')
  OR (table_name = 'sessions' AND field_name = 'title')
`;

const SESSION_CONFLICTS_SQL = `
  SELECT
    e2ee_field_conflicts.id AS id,
    field_name,
    lost_side,
    edited_at_ms,
    value_json,
    e2ee_field_conflicts.created_at AS created_at,
    COALESCE(note.body_format, 'prosemirror_json') AS body_format
  FROM e2ee_field_conflicts
  LEFT JOIN session_documents AS note
    ON note.id = e2ee_field_conflicts.row_id
    AND note.kind = 'note'
    AND note.deleted_at IS NULL
  WHERE row_id = ?
    AND resolved_at IS NULL
    AND (${NOTE_CONFLICT_FIELDS_SQL})
  ORDER BY e2ee_field_conflicts.created_at DESC, e2ee_field_conflicts.id DESC
`;

const SESSION_DOCUMENT_VERSIONS_SQL = `
  SELECT id, body, body_format, source, created_at
  FROM session_document_versions
  WHERE document_id = ?
  ORDER BY created_at DESC, id DESC
  LIMIT 50
`;

export function useSessionConflicts(
  sessionId: string,
): SessionConflictRecord[] {
  const { data = EMPTY_CONFLICTS } = useLiveQuery<
    ConflictSqlRow,
    SessionConflictRecord[]
  >({
    sql: SESSION_CONFLICTS_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapConflictRow),
  });
  return sessionId ? data : EMPTY_CONFLICTS;
}

export function useSessionDocumentVersions(
  sessionId: string,
): SessionDocumentVersionRecord[] {
  const { data = EMPTY_VERSIONS } = useLiveQuery<
    VersionSqlRow,
    SessionDocumentVersionRecord[]
  >({
    sql: SESSION_DOCUMENT_VERSIONS_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapVersionRow),
  });
  return sessionId ? data : EMPTY_VERSIONS;
}

export function resolveSessionConflicts(sessionId: string): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await commands.resolveSessionConflicts({
      session_id: sessionId,
    });
    if (result.status === "error") throw new Error(result.error);
  });
}

export async function applySessionConflict(
  sessionId: string,
  conflict: SessionConflictRecord,
): Promise<void> {
  if (conflict.field === "title") {
    await updateSession(sessionId, { title: conflict.value });
  } else {
    await updateSession(sessionId, {
      raw_md: toEditorBody(conflict.value, conflict.bodyFormat),
    });
  }

  await enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await commands.resolveSessionConflict({
      conflict_id: conflict.id,
    });
    if (result.status === "error") throw new Error(result.error);
  });
}

export function restoreSessionDocumentBody(input: {
  sessionId: string;
  body: string;
  bodyFormat: string;
}): Promise<void> {
  return updateSession(input.sessionId, {
    raw_md: toEditorBody(input.body, input.bodyFormat),
  });
}

export function previewFromBody(body: string, bodyFormat: string): string {
  const text =
    bodyFormat === "prosemirror_json" ? textFromProseMirrorBody(body) : body;
  const collapsed = text.replace(/\s+/g, " ").trim();
  if (collapsed.length <= PREVIEW_LENGTH) return collapsed;
  return `${collapsed.slice(0, PREVIEW_LENGTH).trimEnd()}...`;
}

function textFromProseMirrorBody(body: string): string {
  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    return body;
  }

  const parts: string[] = [];
  const visit = (node: unknown) => {
    if (!node || typeof node !== "object") return;
    const { text, content } = node as { text?: unknown; content?: unknown };
    if (typeof text === "string") parts.push(text);
    if (Array.isArray(content)) {
      content.forEach(visit);
      parts.push(" ");
    }
  };
  visit(parsed);
  return parts.join("");
}

function toEditorBody(body: string, bodyFormat: string): string {
  return bodyFormat === "markdown" ? JSON.stringify(md2json(body)) : body;
}

function mapConflictRow(row: ConflictSqlRow): SessionConflictRecord {
  return {
    id: row.id,
    field: row.field_name === "title" ? "title" : "body",
    lostSide: row.lost_side,
    editedAtMs: row.edited_at_ms ?? null,
    createdAt: row.created_at,
    value: decodeConflictValue(row.value_json),
    bodyFormat: row.body_format,
  };
}

function mapVersionRow(row: VersionSqlRow): SessionDocumentVersionRecord {
  return {
    id: row.id,
    body: row.body,
    bodyFormat: row.body_format,
    source: row.source,
    createdAt: row.created_at,
  };
}

function decodeConflictValue(valueJson: string): string {
  try {
    const parsed = JSON.parse(valueJson);
    return typeof parsed === "string" ? parsed : valueJson;
  } catch {
    return valueJson;
  }
}
