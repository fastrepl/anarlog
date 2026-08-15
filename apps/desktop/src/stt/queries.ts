import type {
  LiveTranscriptDelta,
  RenderTranscriptHuman,
} from "@anlg/plugin-transcription";
import { commands as transcriptionCommands } from "@anlg/plugin-transcription";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import type { SegmentKey } from "~/stt/live-segment";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import {
  applyLiveTranscriptDelta,
  createTranscriptAccumulator,
  parseTranscriptWords,
  parseTranscriptHints,
  updateTranscriptHints,
  updateTranscriptWords,
  upsertSpeakerAssignment,
} from "~/stt/utils";

type TranscriptSqlRow = {
  id: string;
  owner_user_id: string;
  session_id: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  words_json: string;
  speaker_hints_json: string;
  content_revision: number;
  pending_deltas_json: string;
};

type TranscriptMetadataSqlRow = {
  id: string;
  session_id: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  has_words: number;
};

type ParticipantHumanSqlRow = { human_id: string };
type HumanSqlRow = { id: string; name: string };
type TranscriptMutationSqlRow = {
  words_json: string;
  speaker_hints_json: string;
  content_revision: number;
  pending_deltas_json: string;
};

type TranscriptInsert = {
  id: string;
  sessionId: string;
  ownerUserId: string;
  createdAt: string;
  startedAt: number;
  endedAt?: number;
  memo?: string;
  source?: string;
  provider?: string;
  model?: string;
  language?: string;
  words?: WordWithId[];
  speakerHints?: SpeakerHintWithId[];
  replaceSession?: boolean;
  replaceTranscriptId?: string;
};

export type TranscriptRecord = {
  id: string;
  ownerUserId: string;
  sessionId: string;
  startedAt: number;
  endedAt?: number;
  words: WordWithId[];
  speakerHints: SpeakerHintWithId[];
};

export type TranscriptMetadata = {
  id: string;
  sessionId: string;
  startedAt: number;
  endedAt?: number;
  hasWords: boolean;
};

const EMPTY_TRANSCRIPTS: TranscriptRecord[] = [];
const EMPTY_TRANSCRIPT_METADATA: TranscriptMetadata[] = [];
const EMPTY_IDS: string[] = [];
const EMPTY_HUMANS: RenderTranscriptHuman[] = [];

const TRANSCRIPT_COLUMNS = `
  transcript.id,
  transcript.owner_user_id,
  transcript.session_id,
  transcript.started_at_ms,
  transcript.ended_at_ms,
  transcript.words_json,
  transcript.speaker_hints_json,
  transcript.content_revision,
  COALESCE((
    SELECT json_group_array(json(ordered_delta.delta_json))
    FROM (
      SELECT delta.delta_json
      FROM transcript_live_deltas AS delta
      WHERE delta.transcript_id = transcript.id
      ORDER BY delta.sequence
    ) AS ordered_delta
  ), '[]') AS pending_deltas_json
`;

const TRANSCRIPT_METADATA_COLUMNS = `
  transcript.id,
  transcript.session_id,
  transcript.started_at_ms,
  transcript.ended_at_ms,
  CASE
    WHEN trim(transcript.words_json) NOT IN ('', '[]', '{}', 'null')
    OR EXISTS (
      SELECT 1
      FROM transcript_live_deltas AS delta
      WHERE delta.transcript_id = transcript.id
        AND json_valid(delta.delta_json)
        AND (
          COALESCE(json_array_length(delta.delta_json, '$.new_words'), 0) > 0
          OR COALESCE(json_array_length(delta.delta_json, '$.partials'), 0) > 0
        )
    ) THEN 1
    ELSE 0
  END AS has_words
`;

export function useSessionTranscripts(sessionId: string): TranscriptRecord[] {
  const { data = EMPTY_TRANSCRIPTS } = useLiveQuery<
    TranscriptSqlRow,
    TranscriptRecord[]
  >({
    sql: `
      SELECT ${TRANSCRIPT_COLUMNS}
      FROM transcripts AS transcript
      WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
      ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id
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
      FROM transcripts AS transcript
      WHERE transcript.id = ? AND transcript.deleted_at IS NULL
      LIMIT 1
    `,
    params: [transcriptId],
    enabled: Boolean(transcriptId),
    mapRows: (rows) => (rows[0] ? mapTranscriptRow(rows[0]) : null),
  });
  return transcriptId ? data : null;
}

export function useSessionTranscriptMetadata(
  sessionId: string,
): TranscriptMetadata[] {
  const { data = EMPTY_TRANSCRIPT_METADATA } = useLiveQuery<
    TranscriptMetadataSqlRow,
    TranscriptMetadata[]
  >({
    sql: `
      SELECT ${TRANSCRIPT_METADATA_COLUMNS}
      FROM transcripts AS transcript
      WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
      ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapTranscriptMetadataRow),
  });
  return sessionId ? data : EMPTY_TRANSCRIPT_METADATA;
}

export function useTranscriptMetadata(
  transcriptId: string,
): TranscriptMetadata | null {
  const { data = null } = useLiveQuery<
    TranscriptMetadataSqlRow,
    TranscriptMetadata | null
  >({
    sql: `
      SELECT ${TRANSCRIPT_METADATA_COLUMNS}
      FROM transcripts AS transcript
      WHERE transcript.id = ? AND transcript.deleted_at IS NULL
      LIMIT 1
    `,
    params: [transcriptId],
    enabled: Boolean(transcriptId),
    mapRows: (rows) => (rows[0] ? mapTranscriptMetadataRow(rows[0]) : null),
  });
  return transcriptId ? data : null;
}

export async function getTranscriptRecord(
  transcriptId: string,
): Promise<TranscriptRecord | null> {
  if (!transcriptId) {
    return null;
  }

  const rows = await liveQueryClient.execute<TranscriptSqlRow>(
    `
      SELECT ${TRANSCRIPT_COLUMNS}
      FROM transcripts AS transcript
      WHERE transcript.id = ? AND transcript.deleted_at IS NULL
      LIMIT 1
    `,
    [transcriptId],
  );

  return rows[0] ? mapTranscriptRow(rows[0]) : null;
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

export function createTranscript(input: TranscriptInsert): Promise<void> {
  return enqueueDatabaseWrite(`transcript:${input.id}`, async () => {
    const now = new Date().toISOString();
    const statements: Array<{ sql: string; params: unknown[] }> = [];

    if (input.replaceSession) {
      statements.push({
        sql: `
          UPDATE transcripts
          SET deleted_at = ?, updated_at = ?
          WHERE session_id = ? AND deleted_at IS NULL
        `,
        params: [now, now, input.sessionId],
      });
    } else if (input.replaceTranscriptId) {
      statements.push({
        sql: `
          UPDATE transcripts
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND session_id = ? AND deleted_at IS NULL
        `,
        params: [now, now, input.replaceTranscriptId, input.sessionId],
      });
    }

    statements.push({
      sql: `
        INSERT INTO transcripts (
          id, workspace_id, owner_user_id, session_id, source, provider,
          model, language, started_at_ms, ended_at_ms, audio_attachment_id,
          memo, words_json, speaker_hints_json, metadata_json, created_at,
          updated_at, deleted_at
        )
        SELECT ?, session.workspace_id,
          COALESCE(NULLIF(?, ''), session.owner_user_id),
          session.id, ?, ?, ?, ?, ?, ?, '',
          ?, ?, ?, '{}', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
      `,
      params: [
        input.id,
        input.ownerUserId,
        input.source ?? "",
        input.provider ?? "",
        input.model ?? "",
        input.language ?? "",
        input.startedAt,
        input.endedAt ?? null,
        input.memo ?? "",
        JSON.stringify(input.words ?? []),
        JSON.stringify(input.speakerHints ?? []),
        input.createdAt,
        now,
        input.sessionId,
      ],
    });

    await executeTransaction(statements);
  });
}

export function createLiveTranscript(
  input: Omit<TranscriptInsert, "words" | "speakerHints">,
  delta: LiveTranscriptDelta,
): Promise<void> {
  const snapshot = mutateTranscriptSnapshot("[]", "[]", input.id, (store) =>
    applyLiveTranscriptDelta(store, input.id, delta),
  );

  return createTranscript({
    ...input,
    words: JSON.parse(snapshot.wordsJson) as WordWithId[],
    speakerHints: JSON.parse(snapshot.hintsJson) as SpeakerHintWithId[],
  });
}

export async function transcriptExists(transcriptId: string): Promise<boolean> {
  const rows = await liveQueryClient.execute<{ id: string }>(
    `
      SELECT id
      FROM transcripts
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [transcriptId],
  );
  return Boolean(rows[0]);
}

export function applyLiveTranscriptDeltaToDatabase(
  transcriptId: string,
  delta: LiveTranscriptDelta,
): Promise<void> {
  return enqueueDatabaseWrite(`transcript:${transcriptId}`, async () => {
    const now = new Date().toISOString();
    const journalId = `${transcriptId}:${crypto.randomUUID()}`;
    const [, inserted = 0] = await executeTransaction([
      {
        sql: `
          INSERT OR IGNORE INTO transcript_live_state (
            transcript_id, next_sequence, updated_at
          )
          SELECT id, 0, ?
          FROM transcripts
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, transcriptId],
      },
      {
        sql: `
          INSERT INTO transcript_live_deltas (
            id, transcript_id, sequence, delta_json, created_at
          )
          SELECT ?, transcript_id, next_sequence, ?, ?
          FROM transcript_live_state
          WHERE transcript_id = ?
        `,
        params: [journalId, JSON.stringify(delta), now, transcriptId],
      },
      {
        sql: `
          UPDATE transcript_live_state
          SET next_sequence = next_sequence + 1, updated_at = ?
          WHERE transcript_id = ?
        `,
        params: [now, transcriptId],
      },
    ]);

    if (inserted !== 1) {
      throw new Error(`Transcript ${transcriptId} does not exist`);
    }
  });
}

export function flushLiveTranscriptDeltasToDatabase(
  transcriptId: string,
): Promise<void> {
  return mutateTranscript(transcriptId);
}

export function appendTranscriptWordsAndHints(
  transcriptId: string,
  words: WordWithId[],
  hints: SpeakerHintWithId[],
  options?: { mode?: "append" | "replace" },
): Promise<void> {
  return mutateTranscript(transcriptId, (store) => {
    const accumulator = createTranscriptAccumulator(store, transcriptId);
    accumulator.appendWordsAndHints(words, hints, options);
    accumulator.dispose();
  });
}

export function assignTranscriptSpeaker({
  transcriptId,
  segmentKey,
  humanId,
  anchorWordId,
  mode,
  wordIds,
}: {
  transcriptId: string;
  segmentKey: SegmentKey;
  humanId: string;
  anchorWordId: string;
  mode?: "all" | "segment";
  wordIds?: string[];
}): Promise<void> {
  return mutateTranscript(transcriptId, (store) => {
    upsertSpeakerAssignment(
      store,
      transcriptId,
      segmentKey,
      humanId,
      anchorWordId,
      { mode, wordIds },
    );
  }).then(() => {
    if ((mode ?? "all") !== "all") {
      return;
    }
    const channel =
      segmentKey.channel === "DirectMic"
        ? 0
        : segmentKey.channel === "RemoteParty"
          ? 1
          : 2;
    transcriptionCommands
      .promoteVoiceprintCandidates(
        transcriptId,
        channel,
        typeof segmentKey.speaker_index === "number"
          ? segmentKey.speaker_index
          : null,
        humanId,
      )
      .then((result) => {
        if (result.status === "error") {
          console.error("[voiceprint] promotion failed", result.error);
        }
      })
      .catch((error) => {
        console.error("[voiceprint] promotion failed", error);
      });
  });
}

export function updateTranscriptSegmentText({
  transcriptId,
  wordIds,
  text,
}: {
  transcriptId: string;
  wordIds: string[];
  text: string;
}): Promise<void> {
  return mutateTranscript(transcriptId, (store) => {
    const selectedWordIds = new Set(wordIds);
    const words = parseTranscriptWords(store, transcriptId);
    const selectedWords = words.filter((word) => selectedWordIds.has(word.id));
    if (selectedWords.length === 0) {
      return;
    }

    const tokens = text.match(/\S+/g) ?? [];
    const textByWordId = new Map<string, string>();
    for (const [index, word] of selectedWords.entries()) {
      const isLastWord = index === selectedWords.length - 1;
      textByWordId.set(
        word.id,
        isLastWord ? tokens.slice(index).join(" ") : (tokens[index] ?? ""),
      );
    }

    updateTranscriptWords(
      store,
      transcriptId,
      words.map((word) => {
        const nextText = textByWordId.get(word.id);
        return nextText === undefined || nextText === word.text
          ? word
          : { ...word, text: nextText };
      }),
    );
  });
}

export function softDeleteTranscript(transcriptId: string): Promise<void> {
  return enqueueDatabaseWrite(`transcript:${transcriptId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE transcripts
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, transcriptId],
      },
    ]);
  });
}

export async function removeHumanSpeakerAssignments(
  sessionId: string,
  humanId: string,
): Promise<void> {
  const transcripts = await liveQueryClient.execute<{ id: string }>(
    `
      SELECT id
      FROM transcripts
      WHERE session_id = ? AND deleted_at IS NULL
      ORDER BY started_at_ms, created_at, id
    `,
    [sessionId],
  );

  await Promise.all(
    transcripts.map((transcript) =>
      mutateTranscript(transcript.id, (store) => {
        const hints = parseTranscriptHints(store, transcript.id);
        const filtered = hints.filter(
          (hint) =>
            (hint.type !== "automatic_speaker_assignment" &&
              hint.type !== "user_speaker_assignment") ||
            parseAssignedHumanId(hint.value) !== humanId,
        );
        if (filtered.length !== hints.length) {
          updateTranscriptHints(store, transcript.id, filtered);
        }
      }),
    ),
  );
}

function mapTranscriptRow(row: TranscriptSqlRow): TranscriptRecord {
  const snapshot = materializeTranscriptSnapshot(
    row.words_json,
    row.speaker_hints_json,
    row.id,
    row.pending_deltas_json,
  );
  return {
    id: row.id,
    ownerUserId: row.owner_user_id,
    sessionId: row.session_id,
    startedAt: Number(row.started_at_ms),
    endedAt: row.ended_at_ms === null ? undefined : Number(row.ended_at_ms),
    words: parseJsonArray(snapshot.wordsJson, row.id, "words"),
    speakerHints: parseJsonArray(snapshot.hintsJson, row.id, "speaker hints"),
  };
}

function mapTranscriptMetadataRow(
  row: TranscriptMetadataSqlRow,
): TranscriptMetadata {
  return {
    id: row.id,
    sessionId: row.session_id,
    startedAt: Number(row.started_at_ms),
    endedAt: row.ended_at_ms === null ? undefined : Number(row.ended_at_ms),
    hasWords: row.has_words === 1,
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

async function mutateTranscript(
  transcriptId: string,
  mutation?: (store: MemoryTranscriptStore) => void,
): Promise<void> {
  return enqueueDatabaseWrite(`transcript:${transcriptId}`, async () => {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const rows = await liveQueryClient.execute<TranscriptMutationSqlRow>(
        `
          SELECT
            transcript.words_json,
            transcript.speaker_hints_json,
            transcript.content_revision,
            COALESCE((
              SELECT json_group_array(json(ordered_delta.delta_json))
              FROM (
                SELECT delta.delta_json
                FROM transcript_live_deltas AS delta
                WHERE delta.transcript_id = transcript.id
                ORDER BY delta.sequence
              ) AS ordered_delta
            ), '[]') AS pending_deltas_json
          FROM transcripts AS transcript
          WHERE transcript.id = ? AND transcript.deleted_at IS NULL
          LIMIT 1
        `,
        [transcriptId],
      );
      const current = rows[0];
      if (!current) {
        if (!mutation) return;
        throw new Error(`Transcript ${transcriptId} does not exist`);
      }

      assertJsonArray(current.words_json, transcriptId, "words");
      assertJsonArray(
        current.speaker_hints_json,
        transcriptId,
        "speaker hints",
      );
      const pendingDeltas = parseLiveTranscriptDeltas(
        current.pending_deltas_json,
        transcriptId,
      );
      if (!mutation && pendingDeltas.length === 0) return;

      const materialized = materializeTranscriptSnapshot(
        current.words_json,
        current.speaker_hints_json,
        transcriptId,
        current.pending_deltas_json,
      );
      const next = mutation
        ? mutateTranscriptSnapshot(
            materialized.wordsJson,
            materialized.hintsJson,
            transcriptId,
            mutation,
          )
        : materialized;
      const now = new Date().toISOString();
      const [updated = 0] = await executeTransaction([
        {
          sql: `
            UPDATE transcripts
            SET words_json = ?,
              speaker_hints_json = ?,
              content_revision = content_revision + 1,
              updated_at = ?
            WHERE id = ?
              AND content_revision = ?
              AND deleted_at IS NULL
          `,
          params: [
            next.wordsJson,
            next.hintsJson,
            now,
            transcriptId,
            Number(current.content_revision ?? 0),
          ],
        },
        {
          sql: `
            DELETE FROM transcript_live_state
            WHERE transcript_id = ? AND changes() = 1
          `,
          params: [transcriptId],
        },
      ]);

      if (updated === 1) return;
    }

    throw new Error(`Transcript ${transcriptId} changed too frequently`);
  });
}

function materializeTranscriptSnapshot(
  wordsJson: string,
  hintsJson: string,
  transcriptId: string,
  pendingDeltasJson: string,
) {
  let snapshot = { wordsJson, hintsJson };
  for (const delta of parseLiveTranscriptDeltas(
    pendingDeltasJson,
    transcriptId,
  )) {
    snapshot = mutateTranscriptSnapshot(
      snapshot.wordsJson,
      snapshot.hintsJson,
      transcriptId,
      (store) => applyLiveTranscriptDelta(store, transcriptId, delta),
    );
  }
  return snapshot;
}

function parseLiveTranscriptDeltas(
  value: string | undefined,
  transcriptId: string,
): LiveTranscriptDelta[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) return parsed as LiveTranscriptDelta[];
  } catch (error) {
    console.error(
      `[transcript] failed to parse live deltas for ${transcriptId}`,
      error,
    );
  }
  return [];
}

type MemoryTranscriptStore = {
  getCell: (
    tableId: "transcripts",
    rowId: string,
    cellId: "words" | "speaker_hints",
  ) => string;
  setCell: (
    tableId: "transcripts",
    rowId: string,
    cellId: "words" | "speaker_hints",
    value: string,
  ) => void;
};

function mutateTranscriptSnapshot(
  wordsJson: string,
  hintsJson: string,
  transcriptId: string,
  mutation: (store: MemoryTranscriptStore) => void,
) {
  const snapshot = { wordsJson, hintsJson };
  const store: MemoryTranscriptStore = {
    getCell: (_tableId, rowId, cellId) => {
      if (rowId !== transcriptId) return "[]";
      return cellId === "words" ? snapshot.wordsJson : snapshot.hintsJson;
    },
    setCell: (_tableId, rowId, cellId, value) => {
      if (rowId !== transcriptId) return;
      if (cellId === "words") {
        snapshot.wordsJson = value;
      } else {
        snapshot.hintsJson = value;
      }
    },
  };

  mutation(store);
  return snapshot;
}

function assertJsonArray(value: string, rowId: string, field: string): void {
  try {
    if (Array.isArray(JSON.parse(value))) return;
  } catch {
    // Report the same corruption error for malformed and non-array payloads.
  }

  throw new Error(`Transcript ${rowId} has invalid ${field} data`);
}

function parseAssignedHumanId(value: unknown): string | undefined {
  let parsed = value;
  if (typeof value === "string") {
    try {
      parsed = JSON.parse(value);
    } catch {
      return undefined;
    }
  }

  if (!parsed || typeof parsed !== "object" || !("human_id" in parsed)) {
    return undefined;
  }

  const humanId = (parsed as { human_id?: unknown }).human_id;
  return typeof humanId === "string" ? humanId : undefined;
}
