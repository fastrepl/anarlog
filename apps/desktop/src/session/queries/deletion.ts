import { json2md } from "@anlg/editor/markdown";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands } from "@anlg/plugin-session";

import { liveQueryClient } from "~/db";
import { waitForPendingSoftDelete } from "~/session/pending-soft-deletes";
import type { DeletedSessionData } from "~/store/zustand/undo-delete";

type SessionIdentitySqlRow = { id: string };
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
  const result = await commands.softDeleteSession({
    session_id: sessionId,
    tombstone,
  });
  if (result.status === "error") throw new Error(result.error);
  if (!result.data) return null;

  return {
    session: { id: result.data.id, title: result.data.title },
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
            AND kind IN ('meeting_chat', 'meeting_screen')
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
    const result = await commands.restoreDeletedSession({
      session_id: data.session.id,
      tombstone: data.tombstone,
    });
    if (result.status === "error") throw new Error(result.error);
    if (result.data !== "not_deleted") return;

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
