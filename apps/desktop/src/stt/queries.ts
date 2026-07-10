import type { RenderTranscriptHuman } from "@hypr/plugin-transcription";

import { useLiveQuery } from "~/db";
import type { TranscriptRow } from "~/stt/render-transcript";

type TranscriptSqlRow = {
  id: string;
  owner_user_id: string;
  session_id: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  words_json: string;
  speaker_hints_json: string;
};

type ParticipantHumanSqlRow = { human_id: string };
type HumanSqlRow = { id: string; name: string };

export type TranscriptRecord = {
  id: string;
  ownerUserId: string;
  sessionId: string;
  startedAt: number;
  endedAt?: number;
  words: NonNullable<TranscriptRow["words"]>;
  speakerHints: NonNullable<TranscriptRow["speaker_hints"]>;
};

const EMPTY_TRANSCRIPTS: TranscriptRecord[] = [];
const EMPTY_IDS: string[] = [];
const EMPTY_HUMANS: RenderTranscriptHuman[] = [];

const TRANSCRIPT_COLUMNS = `
  id,
  owner_user_id,
  session_id,
  started_at_ms,
  ended_at_ms,
  words_json,
  speaker_hints_json
`;

export function useSessionTranscripts(sessionId: string): TranscriptRecord[] {
  const { data = EMPTY_TRANSCRIPTS } = useLiveQuery<
    TranscriptSqlRow,
    TranscriptRecord[]
  >({
    sql: `
      SELECT ${TRANSCRIPT_COLUMNS}
      FROM transcripts
      WHERE session_id = ? AND deleted_at IS NULL
      ORDER BY started_at_ms, created_at, id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapTranscriptRow),
  });
  return sessionId ? data : EMPTY_TRANSCRIPTS;
}

export function useTranscript(transcriptId: string): TranscriptRecord | null {
  const { data = null } = useLiveQuery<
    TranscriptSqlRow,
    TranscriptRecord | null
  >({
    sql: `
      SELECT ${TRANSCRIPT_COLUMNS}
      FROM transcripts
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [transcriptId],
    enabled: Boolean(transcriptId),
    mapRows: (rows) => (rows[0] ? mapTranscriptRow(rows[0]) : null),
  });
  return transcriptId ? data : null;
}

export function useSessionParticipantHumanIds(sessionId: string): string[] {
  const { data = EMPTY_IDS } = useLiveQuery<ParticipantHumanSqlRow, string[]>({
    sql: `
      SELECT DISTINCT human_id
      FROM session_participants
      WHERE session_id = ?
        AND human_id <> ''
        AND deleted_at IS NULL
      ORDER BY human_id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map((row) => row.human_id),
  });
  return sessionId ? data : EMPTY_IDS;
}

export function useTranscriptHumans(
  humanIds: readonly string[],
): RenderTranscriptHuman[] {
  const uniqueIds = [...new Set(humanIds.filter(Boolean))].sort();
  const placeholders = uniqueIds.map(() => "?").join(", ");
  const { data = EMPTY_HUMANS } = useLiveQuery<
    HumanSqlRow,
    RenderTranscriptHuman[]
  >({
    sql: `
      SELECT id, name
      FROM humans
      WHERE id IN (${placeholders || "NULL"})
        AND name <> ''
        AND deleted_at IS NULL
      ORDER BY id
    `,
    params: uniqueIds,
    enabled: uniqueIds.length > 0,
    mapRows: (rows) =>
      rows.map((row) => ({ human_id: row.id, name: row.name })),
  });
  return uniqueIds.length > 0 ? data : EMPTY_HUMANS;
}

function mapTranscriptRow(row: TranscriptSqlRow): TranscriptRecord {
  return {
    id: row.id,
    ownerUserId: row.owner_user_id,
    sessionId: row.session_id,
    startedAt: Number(row.started_at_ms),
    endedAt: row.ended_at_ms === null ? undefined : Number(row.ended_at_ms),
    words: parseJsonArray(row.words_json, row.id, "words"),
    speakerHints: parseJsonArray(
      row.speaker_hints_json,
      row.id,
      "speaker hints",
    ),
  };
}

function parseJsonArray<T>(value: string, rowId: string, field: string): T[] {
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) return parsed as T[];
  } catch (error) {
    console.error(`[transcript] failed to parse ${field} for ${rowId}`, error);
  }

  return [];
}
