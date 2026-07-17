import { executeTransaction } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { id } from "~/shared/utils";

export type EnhancerNote = SessionContentSnapshot["enhancedNotes"][number];

export function ensureSummaryDocument(
  sessionId: string,
  templateId?: string,
): Promise<EnhancerNote> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const snapshot = await loadSessionContentSnapshot(sessionId);
    if (!snapshot) {
      throw new Error(`Session ${sessionId} no longer exists`);
    }

    const normalizedTemplateId = templateId ?? "";
    const existing = snapshot.enhancedNotes.find(
      (note) => note.templateId === normalizedTemplateId,
    );
    if (existing) {
      return existing;
    }

    const noteId = id();
    const position =
      snapshot.enhancedNotes.reduce(
        (highest, note) => Math.max(highest, note.position),
        0,
      ) + 1;
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          INSERT INTO session_documents (
            id, workspace_id, session_id, kind, template_id, title,
            body_format, body, sort_order, created_by, updated_by, created_at,
            updated_at, deleted_at
          )
          SELECT
            ?, workspace_id, id, ?, ?, 'Summary', 'prosemirror_json', '', ?,
            owner_user_id, owner_user_id, ?, ?, NULL
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [
          noteId,
          normalizedTemplateId ? "template_output" : "summary",
          normalizedTemplateId,
          position,
          now,
          now,
          sessionId,
        ],
        expectedRowsAffected: 1,
      },
    ]);

    return {
      id: noteId,
      title: "Summary",
      markdown: "",
      content: "",
      contentFormat: "prosemirror_json",
      templateId: normalizedTemplateId,
      position,
    };
  });
}

export function replaceSummaryDocumentTemplate({
  sessionId,
  noteId,
  templateId,
  title,
}: {
  sessionId: string;
  noteId: string;
  templateId?: string;
  title: string;
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const normalizedTemplateId = templateId ?? "";
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_documents
          SET
            kind = ?,
            template_id = ?,
            title = ?,
            body_format = 'prosemirror_json',
            body = '',
            updated_by = COALESCE((
              SELECT owner_user_id FROM sessions
              WHERE sessions.id = ? AND sessions.deleted_at IS NULL
            ), updated_by),
            updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND kind IN ('summary', 'template_output')
            AND deleted_at IS NULL
            AND EXISTS (
              SELECT 1 FROM sessions
              WHERE sessions.id = ? AND sessions.deleted_at IS NULL
            )
        `,
        params: [
          normalizedTemplateId ? "template_output" : "summary",
          normalizedTemplateId,
          title,
          sessionId,
          now,
          noteId,
          sessionId,
          sessionId,
        ],
        expectedRowsAffected: 1,
      },
    ]);
  });
}

export function updateSummaryDocumentTitleIfCurrent({
  sessionId,
  noteId,
  templateId,
  currentTitle,
  nextTitle,
}: {
  sessionId: string;
  noteId: string;
  templateId: string;
  currentTitle: string;
  nextTitle: string;
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_documents
          SET title = ?, updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND kind IN ('summary', 'template_output')
            AND template_id = ?
            AND title = ?
            AND deleted_at IS NULL
        `,
        params: [nextTitle, now, noteId, sessionId, templateId, currentTitle],
      },
    ]);
  });
}
