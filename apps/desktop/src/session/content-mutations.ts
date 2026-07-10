import { executeTransaction } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

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
