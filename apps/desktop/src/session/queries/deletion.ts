import { json2md } from "@anlg/editor/markdown";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { executeTransaction, liveQueryClient } from "~/db";
import { waitForPendingSoftDelete } from "~/session/pending-soft-deletes";
import type { DeletedSessionData } from "~/store/zustand/undo-delete";

type SessionIdentitySqlRow = { id: string };
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

export async function softDeleteSession(
  sessionId: string,
  tombstone = new Date().toISOString(),
): Promise<DeletedSessionData | null> {
  const [session] = await liveQueryClient.execute<SessionDeleteSqlRow>(
    `SELECT id, title FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1`,
    [sessionId],
  );
  if (!session) return null;

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

export async function isSessionDeleted(sessionId: string): Promise<boolean> {
  await waitForPendingSoftDelete(sessionId);
  const [session] = await liveQueryClient.execute<SessionIdentitySqlRow>(
    `SELECT id FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1`,
    [sessionId],
  );
  return !session;
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
  // The undo toast shows before the soft-delete write commits. Wait for the
  // in-flight delete to settle first — an "alive" session during that window
  // is not restored, it just isn't tombstoned yet.
  await waitForPendingSoftDelete(data.session.id);
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const rowsAffected = await executeTransaction(
      buildSessionTombstoneStatements(data.session.id, data.tombstone, true),
    );
    if (rowsAffected[rowsAffected.length - 1] === 1) return;

    const [alive] = await liveQueryClient.execute<SessionIdentitySqlRow>(
      `SELECT id FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1`,
      [data.session.id],
    );
    if (alive) return;

    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  throw new Error(`Session ${data.session.id} was never soft-deleted`);
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
