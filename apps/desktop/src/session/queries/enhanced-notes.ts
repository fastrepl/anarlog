import { useCallback } from "react";

import { md2json } from "@anlg/editor/markdown";

import type { EnhancedNoteRecord } from "./types";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

type EnhancedNoteSqlRow = {
  id: string;
  session_id: string;
  title: string;
  body: string;
  body_format: string;
  template_id: string;
  sort_order: number;
};

const PENDING_AUTO_ENHANCE_SETTING_PREFIX = "auto_enhance_pending:";
const EMPTY_ENHANCED_NOTES: EnhancedNoteRecord[] = [];

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
      {
        sql: `
          DELETE FROM app_settings
          WHERE id = ?
            AND json_valid(value_json)
            AND json_extract(
              CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END,
              '$.noteId'
            ) = ?
            AND json_extract(
              CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END,
              '$.body'
            ) <> ?
        `,
        params: [
          `${PENDING_AUTO_ENHANCE_SETTING_PREFIX}${sessionId}`,
          enhancedNoteId,
          content,
        ],
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
