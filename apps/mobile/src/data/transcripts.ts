import { useMemo } from "react";

import { renderStoredTranscript } from "@anlg/mobile-bridge";

import { execute, executeTransaction, useLiveQuery } from "@/db";
import { captureOperationalError } from "@/lib/error-reporting";

import {
  applyTranscriptEdits,
  SESSION_HAS_TRANSCRIPT_SQL,
  SESSION_SPEAKERS_SQL,
  SESSION_TRANSCRIPTS_SQL,
  transcriptEditPatch,
  transcriptEditStatements,
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

export function renderTranscript(
  row: TranscriptRow,
  humans: { id: string; name: string }[],
): TranscriptSegment[] {
  const segments = JSON.parse(
    renderStoredTranscript(
      JSON.stringify({
        ...row,
        humans: humans.map(({ id, name }) => ({ human_id: id, name })),
      }),
    ),
  ) as TranscriptSegment[];
  return applyTranscriptEdits(segments, row.metadata_json);
}

export async function saveTranscriptSegmentEdit(
  sessionId: string,
  segment: TranscriptSegment,
  nextText: string,
) {
  const text = nextText.trim();
  if (!text) throw new Error("A transcript segment cannot be empty.");
  if (Array.from(text).length > 20_000)
    throw new Error("Transcript segments are limited to 20,000 characters.");
  const separator = segment.id.indexOf(":");
  if (separator < 1) throw new Error("This transcript segment is unavailable.");
  const transcriptId = segment.id.slice(0, separator);
  const [row] = await execute<{ metadata_json: string }>(
    `SELECT metadata_json FROM transcripts
     WHERE id = ? AND session_id = ? AND deleted_at IS NULL`,
    [transcriptId, sessionId],
  );
  if (!row) throw new Error("This transcript is no longer available.");
  let previous:
    | { text: string; revisions: { text: string; edited_at: string }[] }
    | undefined;
  try {
    const metadata: unknown = JSON.parse(row.metadata_json || "{}");
    if (metadata && typeof metadata === "object") {
      const edits = (metadata as { transcript_edits?: Record<string, unknown> })
        .transcript_edits;
      const edit =
        edits && !Array.isArray(edits) ? edits[segment.id] : undefined;
      if (
        edit &&
        typeof edit === "object" &&
        "text" in edit &&
        typeof edit.text === "string"
      ) {
        const revisions =
          "revisions" in edit && Array.isArray(edit.revisions)
            ? edit.revisions.filter(
                (revision): revision is { text: string; edited_at: string } =>
                  Boolean(
                    revision &&
                    typeof revision === "object" &&
                    "text" in revision &&
                    typeof revision.text === "string" &&
                    "edited_at" in revision &&
                    typeof revision.edited_at === "string",
                  ),
              )
            : [];
        previous = { text: edit.text, revisions };
      }
    }
  } catch {
    // Replacing malformed metadata with a valid edit object is safe; transcript words remain intact.
  }
  if (text === (previous?.text ?? segment.text)) return;
  const updatedAt = new Date().toISOString();
  const patch = transcriptEditPatch(
    segment.id,
    segment.originalText ?? segment.text,
    text,
    updatedAt,
    previous,
  );
  const [changed = 0] = await executeTransaction(
    transcriptEditStatements(
      transcriptId,
      sessionId,
      row.metadata_json,
      JSON.stringify(patch),
      updatedAt,
    ),
  );
  if (changed !== 1) throw new Error("The transcript edit could not be saved.");
}

function mapTranscripts(
  rows: TranscriptRow[],
  humans: { id: string; name: string }[],
) {
  return rows.flatMap((row) => {
    try {
      return renderTranscript(row, humans);
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
}

export function useSessionHasTranscript(sessionId: string) {
  return useLiveQuery<{ has_transcript: number }, boolean>({
    sql: SESSION_HAS_TRANSCRIPT_SQL,
    params: [sessionId],
    mapRows: (rows) => rows[0]?.has_transcript === 1,
  });
}

export async function loadSessionTranscripts(sessionId: string) {
  const rows = await execute<TranscriptRow>(SESSION_TRANSCRIPTS_SQL, [
    sessionId,
  ]);
  if (rows.length === 0) return [];
  const humans = await execute<{ id: string; name: string }>(
    SESSION_SPEAKERS_SQL,
    [sessionId],
  );
  return mapTranscripts(rows, humans);
}

export function useSessionTranscripts(sessionId: string, enabled: boolean) {
  const {
    data: rows,
    isLoading,
    error,
  } = useLiveQuery<TranscriptRow, TranscriptRow[]>({
    sql: SESSION_TRANSCRIPTS_SQL,
    params: [sessionId],
    enabled,
    mapRows: (rows) => rows,
  });
  const {
    data: humans,
    isLoading: speakersLoading,
    error: speakersError,
  } = useLiveQuery<
    { id: string; name: string },
    { id: string; name: string }[]
  >({
    sql: SESSION_SPEAKERS_SQL,
    params: [sessionId],
    enabled: enabled && (rows?.length ?? 0) > 0,
    mapRows: (rows) => rows,
  });
  const segments = useMemo(
    () => mapTranscripts(rows ?? [], humans ?? []),
    [rows, humans],
  );
  return {
    segments,
    isLoading: isLoading || speakersLoading,
    error: error ?? speakersError,
  };
}
