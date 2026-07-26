import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { id } from "~/shared/utils";

export type EnhancerNote = SessionContentSnapshot["enhancedNotes"][number];

const PENDING_AUTO_ENHANCE_SETTING_PREFIX = "auto_enhance_pending:";

export type PendingAutoEnhanceJob = {
  sessionId: string;
  noteId: string;
  templateId: string;
  expectedBody: string;
  expectedContentFormat: string;
  generation: string;
};

export function ensureSummaryDocument(
  sessionId: string,
  templateId?: string,
): Promise<EnhancerNote> {
  return ensureSummaryDocumentWithMetadata(sessionId, templateId, false);
}

export function ensurePendingAutoEnhanceDocument(
  sessionId: string,
  templateId?: string,
): Promise<PendingAutoEnhanceJob> {
  return ensureSummaryDocumentWithMetadata(sessionId, templateId, true);
}

export async function loadPendingAutoEnhanceJobs(): Promise<
  PendingAutoEnhanceJob[]
> {
  const rows = await liveQueryClient.execute<{
    session_id: string;
    note_id: string;
    template_id: string;
    expected_body: string;
    expected_content_format: string;
    generation: string;
  }>(
    `
      SELECT
        substr(setting.id, ?) AS session_id,
        document.id AS note_id,
        document.template_id,
        document.body AS expected_body,
        document.body_format AS expected_content_format,
        json_extract(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.generation'
        ) AS generation
      FROM app_settings AS setting
      JOIN sessions AS session
        ON session.id = substr(setting.id, ?)
        AND session.deleted_at IS NULL
      JOIN session_documents AS document
        ON document.id = json_extract(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.noteId'
        )
        AND document.session_id = session.id
        AND document.body = json_extract(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.body'
        )
      WHERE setting.id LIKE ?
        AND json_valid(setting.value_json)
        AND json_type(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.noteId'
        ) = 'text'
        AND json_type(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.body'
        ) = 'text'
        AND json_type(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.generation'
        ) = 'text'
        AND json_extract(
          CASE
            WHEN json_valid(setting.value_json) THEN setting.value_json
            ELSE '{}'
          END,
          '$.bodyFormat'
        ) = document.body_format
        AND document.kind IN ('summary', 'template_output')
        AND document.deleted_at IS NULL
        AND EXISTS (
          SELECT 1
          FROM transcripts AS transcript
          WHERE transcript.session_id = session.id
            AND transcript.deleted_at IS NULL
            AND json_valid(transcript.words_json)
            AND json_type(transcript.words_json) = 'array'
            AND json_array_length(transcript.words_json) > 0
        )
      ORDER BY setting.updated_at, session_id
    `,
    [
      PENDING_AUTO_ENHANCE_SETTING_PREFIX.length + 1,
      PENDING_AUTO_ENHANCE_SETTING_PREFIX.length + 1,
      `${PENDING_AUTO_ENHANCE_SETTING_PREFIX}%`,
    ],
  );
  return rows.map((row) => ({
    sessionId: row.session_id,
    noteId: row.note_id,
    templateId: row.template_id,
    expectedBody: row.expected_body,
    expectedContentFormat: row.expected_content_format,
    generation: row.generation,
  }));
}

function ensureSummaryDocumentWithMetadata(
  sessionId: string,
  templateId: string | undefined,
  pendingAutoEnhance: false,
): Promise<EnhancerNote>;
function ensureSummaryDocumentWithMetadata(
  sessionId: string,
  templateId: string | undefined,
  pendingAutoEnhance: true,
): Promise<PendingAutoEnhanceJob>;
function ensureSummaryDocumentWithMetadata(
  sessionId: string,
  templateId: string | undefined,
  pendingAutoEnhance: boolean,
): Promise<EnhancerNote | PendingAutoEnhanceJob> {
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
      if (pendingAutoEnhance) {
        const generation = id();
        const now = new Date().toISOString();
        await executeTransaction([
          {
            sql: `
              INSERT INTO app_settings (id, value_json, updated_at)
              VALUES (?, ?, ?)
              ON CONFLICT(id) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at
            `,
            params: [
              `${PENDING_AUTO_ENHANCE_SETTING_PREFIX}${sessionId}`,
              JSON.stringify({
                noteId: existing.id,
                body: existing.content,
                bodyFormat: existing.contentFormat,
                generation,
              }),
              now,
            ],
            expectedRowsAffected: 1,
          },
        ]);
        return {
          sessionId,
          noteId: existing.id,
          templateId: existing.templateId,
          expectedBody: existing.content,
          expectedContentFormat: existing.contentFormat,
          generation,
        };
      }
      return existing;
    }

    const noteId = id();
    const generation = pendingAutoEnhance ? id() : "";
    const position =
      snapshot.enhancedNotes.reduce(
        (highest, note) => Math.max(highest, note.position),
        0,
      ) + 1;
    const now = new Date().toISOString();
    const statements: Array<{
      sql: string;
      params: unknown[];
      expectedRowsAffected: number;
    }> = [
      {
        sql: `
          INSERT INTO session_documents (
            id, workspace_id, session_id, kind, template_id, title,
            body_format, body, sort_order, created_by, updated_by,
            created_at, updated_at, deleted_at
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
    ];

    if (pendingAutoEnhance) {
      statements.push({
        sql: `
          INSERT INTO app_settings (id, value_json, updated_at)
          VALUES (?, ?, ?)
          ON CONFLICT(id) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at = excluded.updated_at
        `,
        params: [
          `${PENDING_AUTO_ENHANCE_SETTING_PREFIX}${sessionId}`,
          JSON.stringify({
            noteId,
            body: "",
            bodyFormat: "prosemirror_json",
            generation,
          }),
          now,
        ],
        expectedRowsAffected: 1,
      });
    }

    await executeTransaction(statements);

    if (pendingAutoEnhance) {
      return {
        sessionId,
        noteId,
        templateId: normalizedTemplateId,
        expectedBody: "",
        expectedContentFormat: "prosemirror_json",
        generation,
      };
    }

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
