import { useLiveQuery } from "@/db";
import { captureOperationalError } from "@/lib/error-reporting";

type TranscriptRow = {
  id: string;
  words_json: string;
};

export type TranscriptSegment = {
  id: string;
  text: string;
};

const MAX_REPORTED_INVALID_ROWS = 1_000;
const reportedInvalidRows = new Set<string>();

function rememberInvalidRow(rowId: string) {
  reportedInvalidRows.add(rowId);
  while (reportedInvalidRows.size > MAX_REPORTED_INVALID_ROWS) {
    const oldestRowId = reportedInvalidRows.values().next().value;
    if (oldestRowId === undefined) {
      break;
    }
    reportedInvalidRows.delete(oldestRowId);
  }
}

function mapTranscriptRows(rows: TranscriptRow[]): TranscriptSegment[] {
  return rows
    .map((row) => {
      let text = "";
      try {
        const words = JSON.parse(row.words_json) as { text?: string }[];
        text = words
          .map((word) => word.text ?? "")
          .join(" ")
          .replace(/\s+/g, " ")
          .trim();
      } catch (error) {
        if (!reportedInvalidRows.has(row.id)) {
          rememberInvalidRow(row.id);
          captureOperationalError(error, {
            operation: "transcript_words_parse",
            level: "warning",
          });
        }
      }
      return { id: row.id, text };
    })
    .filter((segment) => segment.text !== "");
}

export function useSessionTranscripts(sessionId: string): TranscriptSegment[] {
  const { data } = useLiveQuery<TranscriptRow, TranscriptSegment[]>({
    sql: "SELECT id, words_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL ORDER BY started_at_ms, id",
    params: [sessionId],
    mapRows: mapTranscriptRows,
  });
  return data ?? [];
}
