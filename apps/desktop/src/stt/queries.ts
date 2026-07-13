import { useMemo } from "react";

import type {
  LiveTranscriptDelta,
  RenderTranscriptHuman,
} from "@hypr/plugin-transcription";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import type { RenderLabelContext, SegmentKey } from "~/stt/live-segment";
import {
  collectAssignedHumanIdsFromTranscriptRows,
  type TranscriptRow,
} from "~/stt/render-transcript";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import {
  applyLiveTranscriptDelta,
  createTranscriptAccumulator,
  parseTranscriptHints,
  updateTranscriptHints,
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
};

type ParticipantHumanSqlRow = { human_id: string };
type HumanSqlRow = { id: string; name: string };
type TranscriptMutationSqlRow = {
  words_json: string;
  speaker_hints_json: string;
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
};

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

export function useTranscriptLabelContext(
  transcriptId: string,
): RenderLabelContext | undefined {
  const transcript = useTranscript(transcriptId);
  const participantHumanIds = useSessionParticipantHumanIds(
    transcript?.sessionId ?? "",
  );
  const assignedHumanIds = useMemo(
    () =>
      transcript
        ? collectAssignedHumanIdsFromTranscriptRows([
            { speaker_hints: transcript.speakerHints },
          ])
        : EMPTY_IDS,
    [transcript],
  );
  const humanIds = useMemo(
    () => [
      ...new Set([
        ...participantHumanIds,
        ...assignedHumanIds,
        transcript?.ownerUserId ?? "",
      ]),
    ],
    [assignedHumanIds, participantHumanIds, transcript?.ownerUserId],
  );
  const humans = useTranscriptHumans(humanIds);

  return useMemo(() => {
    if (!transcript) return undefined;

    const names = new Map(humans.map((human) => [human.human_id, human.name]));
    return {
      getSelfHumanId: () => transcript.ownerUserId || undefined,
      getHumanName: (humanId) => names.get(humanId),
      getParticipantHumanIds: () => participantHumanIds,
    };
  }, [humans, participantHumanIds, transcript]);
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
    }

    statements.push({
      sql: `
        INSERT INTO transcripts (
          id, workspace_id, owner_user_id, session_id, source, provider,
          model, language, started_at_ms, ended_at_ms, audio_attachment_id,
          memo, words_json, speaker_hints_json, metadata_json, created_at,
          updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, ?, session.id, ?, ?, ?, ?, ?, ?, '',
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

export function applyLiveTranscriptDeltaToDatabase(
  transcriptId: string,
  delta: LiveTranscriptDelta,
): Promise<void> {
  return mutateTranscript(transcriptId, (store) => {
    applyLiveTranscriptDelta(store, transcriptId, delta);
  });
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
            hint.type !== "user_speaker_assignment" ||
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

async function mutateTranscript(
  transcriptId: string,
  mutation: (store: MemoryTranscriptStore) => void,
): Promise<void> {
  return enqueueDatabaseWrite(`transcript:${transcriptId}`, async () => {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const rows = await liveQueryClient.execute<TranscriptMutationSqlRow>(
        `
          SELECT words_json, speaker_hints_json
          FROM transcripts
          WHERE id = ? AND deleted_at IS NULL
          LIMIT 1
        `,
        [transcriptId],
      );
      const current = rows[0];
      if (!current) {
        throw new Error(`Transcript ${transcriptId} does not exist`);
      }

      assertJsonArray(current.words_json, transcriptId, "words");
      assertJsonArray(
        current.speaker_hints_json,
        transcriptId,
        "speaker hints",
      );
      const next = mutateTranscriptSnapshot(
        current.words_json,
        current.speaker_hints_json,
        transcriptId,
        mutation,
      );
      const now = new Date().toISOString();
      const [updated = 0] = await executeTransaction([
        {
          sql: `
            UPDATE transcripts
            SET words_json = ?, speaker_hints_json = ?, updated_at = ?
            WHERE id = ?
              AND words_json = ?
              AND speaker_hints_json = ?
              AND deleted_at IS NULL
          `,
          params: [
            next.wordsJson,
            next.hintsJson,
            now,
            transcriptId,
            current.words_json,
            current.speaker_hints_json,
          ],
        },
      ]);

      if (updated === 1) return;
    }

    throw new Error(`Transcript ${transcriptId} changed too frequently`);
  });
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
