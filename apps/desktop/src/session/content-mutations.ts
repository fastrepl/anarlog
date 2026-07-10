import { executeTransaction } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { DEFAULT_USER_ID } from "~/shared/utils";

export type SummaryContentCorrection = {
  id: string;
  currentContent: string;
  currentContentFormat: string;
  nextContent: string;
};

export type TranscriptContentCorrection = {
  id: string;
  currentWordsJson: string;
  currentMemo: string;
  nextWordsJson: string;
  nextMemo: string;
};

export type SessionDocumentContentUpdate = {
  id: string;
  currentContent: string;
  currentContentFormat: string;
  nextContent: string;
};

export function applySessionContentCorrections({
  sessionId,
  summaries,
  transcripts,
}: {
  sessionId: string;
  summaries: SummaryContentCorrection[];
  transcripts: TranscriptContentCorrection[];
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const statements: Array<{
      sql: string;
      params: unknown[];
      expectedRowsAffected: number;
    }> = [];

    for (const summary of summaries) {
      statements.push({
        sql: `
          UPDATE session_documents
          SET body = ?, body_format = 'prosemirror_json', updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND kind IN ('summary', 'template_output')
            AND body = ?
            AND body_format = ?
            AND deleted_at IS NULL
        `,
        params: [
          summary.nextContent,
          now,
          summary.id,
          sessionId,
          summary.currentContent,
          summary.currentContentFormat,
        ],
        expectedRowsAffected: 1,
      });
    }

    for (const transcript of transcripts) {
      statements.push({
        sql: `
          UPDATE transcripts
          SET words_json = ?, memo = ?, updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND words_json = ?
            AND memo = ?
            AND deleted_at IS NULL
        `,
        params: [
          transcript.nextWordsJson,
          transcript.nextMemo,
          now,
          transcript.id,
          sessionId,
          transcript.currentWordsJson,
          transcript.currentMemo,
        ],
        expectedRowsAffected: 1,
      });
    }

    if (statements.length > 0) await executeTransaction(statements);
  });
}

export function persistGeneratedEnhancedNote({
  sessionId,
  ownerUserId,
  note,
  tagNames,
}: {
  sessionId: string;
  ownerUserId: string;
  note: SessionDocumentContentUpdate;
  tagNames: string[];
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const userId = ownerUserId.trim() || DEFAULT_USER_ID;
    const normalizedTagNames = [...new Set(tagNames)].filter(Boolean);
    const statements: Array<{
      sql: string;
      params: unknown[];
      expectedRowsAffected: number;
    }> = [
      {
        sql: `
          UPDATE session_documents
          SET body = ?, body_format = 'prosemirror_json', updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND kind IN ('summary', 'template_output')
            AND body = ?
            AND body_format = ?
            AND deleted_at IS NULL
            AND EXISTS (
              SELECT 1 FROM sessions
              WHERE sessions.id = ? AND sessions.deleted_at IS NULL
            )
        `,
        params: [
          note.nextContent,
          now,
          note.id,
          sessionId,
          note.currentContent,
          note.currentContentFormat,
          sessionId,
        ],
        expectedRowsAffected: 1,
      },
    ];

    for (const tagName of normalizedTagNames) {
      statements.push(
        {
          sql: `
            INSERT INTO tags (
              id, owner_user_id, name, created_at, updated_at, deleted_at
            ) VALUES (?, ?, ?, ?, ?, NULL)
            ON CONFLICT(id) DO UPDATE SET
              owner_user_id = excluded.owner_user_id,
              name = excluded.name,
              updated_at = excluded.updated_at,
              deleted_at = NULL
          `,
          params: [tagName, userId, tagName, now, now],
          expectedRowsAffected: 1,
        },
        {
          sql: `
            INSERT INTO session_tags (
              id, owner_user_id, session_id, tag_id,
              created_at, updated_at, deleted_at
            ) VALUES (?, ?, ?, ?, ?, ?, NULL)
            ON CONFLICT(id) DO UPDATE SET
              owner_user_id = excluded.owner_user_id,
              session_id = excluded.session_id,
              tag_id = excluded.tag_id,
              updated_at = excluded.updated_at,
              deleted_at = NULL
          `,
          params: [
            `${sessionId}:${tagName}`,
            userId,
            sessionId,
            tagName,
            now,
            now,
          ],
          expectedRowsAffected: 1,
        },
      );
    }

    await executeTransaction(statements);
  });
}

export function applyGeneratedSessionTitle({
  sessionId,
  currentTitle,
  nextTitle,
  documents,
}: {
  sessionId: string;
  currentTitle: string;
  nextTitle: string;
  documents: SessionDocumentContentUpdate[];
}): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    const statements: Array<{
      sql: string;
      params: unknown[];
      expectedRowsAffected: number;
    }> = [
      {
        sql: `
          UPDATE sessions
          SET title = ?, updated_at = ?
          WHERE id = ? AND title = ? AND deleted_at IS NULL
        `,
        params: [nextTitle, now, sessionId, currentTitle],
        expectedRowsAffected: 1,
      },
    ];

    for (const document of documents) {
      statements.push({
        sql: `
          UPDATE session_documents
          SET body = ?, body_format = 'prosemirror_json', updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND kind IN ('note', 'summary', 'template_output')
            AND body = ?
            AND body_format = ?
            AND deleted_at IS NULL
        `,
        params: [
          document.nextContent,
          now,
          document.id,
          sessionId,
          document.currentContent,
          document.currentContentFormat,
        ],
        expectedRowsAffected: 1,
      });
    }

    await executeTransaction(statements);
  });
}
