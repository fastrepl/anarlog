import { eq, sql, transcripts } from "@hypr/db";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteTranscriptRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  session_id: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  words_json: string;
  speaker_hints_json: string;
  memo: string;
  deleted_at: string | null;
};

const TRANSCRIPT_SHADOW_CONFIG: SqliteTableShadowConfig<
  "transcripts",
  SqliteTranscriptRow
> = {
  label: "SqliteTranscriptShadow",
  tableId: "transcripts",
  selectSql: `
    SELECT
      id,
      owner_user_id,
      created_at,
      session_id,
      started_at_ms,
      ended_at_ms,
      words_json,
      speaker_hints_json,
      memo,
      deleted_at
    FROM transcripts
    ORDER BY session_id, created_at, id
  `,
  fromSqlite: (row) =>
    normalizeTranscriptRow({
      user_id: row.owner_user_id,
      created_at: row.created_at,
      session_id: row.session_id,
      started_at: row.started_at_ms,
      ended_at: row.ended_at_ms ?? undefined,
      words: row.words_json,
      speaker_hints: row.speaker_hints_json,
      memo_md: row.memo,
    }),
  normalize: normalizeTranscriptRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistTranscriptChanges,
};

export function SqliteTranscriptShadow({ store }: { store: Store }) {
  return <SqliteTableShadow config={TRANSCRIPT_SHADOW_CONFIG} store={store} />;
}

export function normalizeTranscriptRow(
  row: Partial<MainTableRow<"transcripts">>,
): MainTableRow<"transcripts"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    session_id: typeof row.session_id === "string" ? row.session_id : "",
    started_at: typeof row.started_at === "number" ? row.started_at : 0,
    ended_at: typeof row.ended_at === "number" ? row.ended_at : undefined,
    words: typeof row.words === "string" ? row.words : "[]",
    speaker_hints:
      typeof row.speaker_hints === "string" ? row.speaker_hints : "[]",
    memo_md: typeof row.memo_md === "string" ? row.memo_md : "",
  };
}

async function persistTranscriptChanges(
  rows: Array<[string, MainTableRow<"transcripts">]>,
  deletedIds: string[],
): Promise<void> {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(transcripts)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeTranscriptRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            sessionId: row.session_id,
            source: "",
            provider: "",
            model: "",
            language: "",
            startedAtMs: row.started_at,
            endedAtMs: row.ended_at ?? null,
            audioAttachmentId: "",
            memo: row.memo_md,
            wordsJson: row.words,
            speakerHintsJson: row.speaker_hints,
            metadataJson: "{}",
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: transcripts.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          sessionId: sql`excluded.session_id`,
          startedAtMs: sql`excluded.started_at_ms`,
          endedAtMs: sql`excluded.ended_at_ms`,
          memo: sql`excluded.memo`,
          wordsJson: sql`excluded.words_json`,
          speakerHintsJson: sql`excluded.speaker_hints_json`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  const deletedAt = new Date().toISOString();
  for (const id of deletedIds) {
    await db
      .update(transcripts)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(transcripts.id, id));
  }
}

function chunks<T>(items: T[], size: number): T[][] {
  const result: T[][] = [];
  for (let index = 0; index < items.length; index += size) {
    result.push(items.slice(index, index + size));
  }
  return result;
}
