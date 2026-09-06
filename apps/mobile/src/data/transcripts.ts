import { useMemo } from "react";

import { useLiveQuery } from "@/db";
import { captureOperationalError } from "@/lib/error-reporting";

import {
  transcriptSegments,
  SESSION_TRANSCRIPTS_SQL,
  SESSION_SPEAKERS_SQL,
  type TranscriptRow,
  type TranscriptSegment,
} from "./transcript-model";
export type { TranscriptSegment } from "./transcript-model";

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

export function useSessionTranscripts(sessionId: string): TranscriptSegment[] {
  const { data: rows } = useLiveQuery<TranscriptRow, TranscriptRow[]>({
    sql: SESSION_TRANSCRIPTS_SQL,
    params: [sessionId],
    mapRows: (rows) => rows,
  });
  const { data: humans } = useLiveQuery<
    { id: string; name: string },
    { id: string; name: string }[]
  >({
    sql: SESSION_SPEAKERS_SQL,
    params: [sessionId],
    mapRows: (rows) => rows,
  });
  return useMemo(() => {
    const names = new Map(
      (humans ?? []).map((human) => [human.id, human.name]),
    );
    return (rows ?? []).flatMap((row) => {
      try {
        return transcriptSegments(row, names);
      } catch (error) {
        if (!reportedInvalidRows.has(row.id)) {
          rememberInvalidRow(row.id);
          captureOperationalError(error, {
            operation: "transcript_words_parse",
            level: "warning",
          });
        }
        return [];
      }
    });
  }, [rows, humans]);
}
