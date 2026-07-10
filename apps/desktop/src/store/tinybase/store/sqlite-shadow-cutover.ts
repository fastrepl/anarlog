import { eq, sql, storageMigrationState } from "@hypr/db";

import type { Schemas } from "./main";

import { db } from "~/db";

type MainTableId = keyof Schemas[0] & string;

export const SQLITE_SHADOWED_TABLES = [
  "sessions",
  "transcripts",
  "humans",
  "organizations",
  "mapping_session_participant",
  "tags",
  "mapping_tag_session",
  "mapping_mention",
  "enhanced_notes",
  "session_key_facts",
] as const satisfies readonly MainTableId[];

const expectedTables = new Set<MainTableId>(SQLITE_SHADOWED_TABLES);
const readyTables = new Set<MainTableId>();
let markCutoverPromise: Promise<void> | undefined;

export function markSqliteShadowReady(tableId: MainTableId): Promise<void> {
  if (!expectedTables.has(tableId)) {
    return Promise.reject(
      new Error(`unexpected SQLite shadow table: ${tableId}`),
    );
  }

  readyTables.add(tableId);
  if (readyTables.size !== expectedTables.size) {
    return Promise.resolve();
  }

  markCutoverPromise ??= markStorageCutover().catch((error) => {
    markCutoverPromise = undefined;
    throw error;
  });
  return markCutoverPromise;
}

async function markStorageCutover(): Promise<void> {
  await db
    .update(storageMigrationState)
    .set({
      phase: "sqlite",
      cutoverAt: sql`COALESCE(${storageMigrationState.cutoverAt}, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))`,
      lastError: "",
      updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
    })
    .where(eq(storageMigrationState.id, "legacy_v1"));
}
