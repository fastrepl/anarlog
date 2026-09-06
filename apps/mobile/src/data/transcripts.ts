import { useMemo } from "react";

import { useLiveQuery } from "@/db";
import { captureOperationalError } from "@/lib/error-reporting";

import {
  transcriptSegments,
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
    sql: `SELECT transcript.id, transcript.started_at_ms, transcript.words_json, transcript.speaker_hints_json,
      COALESCE((SELECT json_group_array(json(ordered_delta.delta_json)) FROM (
        SELECT delta.delta_json FROM transcript_live_deltas AS delta
        WHERE delta.transcript_id = transcript.id ORDER BY delta.sequence
      ) AS ordered_delta), '[]') AS pending_deltas_json
      FROM transcripts AS transcript WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
      ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id`,
    params: [sessionId],
    mapRows: (rows) => rows,
  });
  const { data: humans } = useLiveQuery<
    { id: string; name: string },
    { id: string; name: string }[]
  >({
    sql: `SELECT id, name FROM humans WHERE workspace_id = (SELECT workspace_id FROM sessions WHERE id = ?) AND deleted_at IS NULL`,
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
