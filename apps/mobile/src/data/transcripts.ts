import { useLiveQuery } from "@/db";

type TranscriptRow = {
  id: string;
  words_json: string;
};

export type TranscriptSegment = {
  id: string;
  text: string;
};

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
      } catch {}
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
