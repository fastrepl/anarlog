import { actionItems, entityMentions, eq, sql } from "@hypr/db";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteTaskRow = {
  id: string;
  created_by: string;
  session_id: string;
  source_type: string;
  source_id: string;
  source_order: number;
  status: string;
  text: string;
  body_json: string;
  due_at: string;
  deleted_at: string | null;
};

type SqliteMentionRow = {
  id: string;
  owner_user_id: string;
  source_type: string;
  source_id: string;
  target_type: string;
  target_id: string;
  deleted_at: string | null;
};

const TASK_SHADOW_CONFIG: SqliteTableShadowConfig<"tasks", SqliteTaskRow> = {
  label: "SqliteTaskShadow",
  tableId: "tasks",
  selectSql: `
    SELECT
      id,
      created_by,
      session_id,
      source_type,
      source_id,
      source_order,
      status,
      text,
      body_json,
      due_at,
      deleted_at
    FROM action_items
    ORDER BY source_type, source_id, source_order, id
  `,
  fromSqlite: (row) =>
    normalizeTaskRow({
      user_id: row.created_by,
      task_id: row.id,
      source_id: row.source_id || row.session_id,
      source_type: row.source_type,
      source_order: row.source_order,
      status: row.status,
      text_preview: row.text,
      body_json: row.body_json,
      due_date: row.due_at,
    }),
  normalize: normalizeTaskRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistTaskChanges,
};

const MENTION_SHADOW_CONFIG: SqliteTableShadowConfig<
  "mapping_mention",
  SqliteMentionRow
> = {
  label: "SqliteMentionShadow",
  tableId: "mapping_mention",
  selectSql: `
    SELECT
      id,
      owner_user_id,
      source_type,
      source_id,
      target_type,
      target_id,
      deleted_at
    FROM entity_mentions
    ORDER BY source_type, source_id, target_type, target_id, id
  `,
  fromSqlite: (row) =>
    normalizeMentionRow({
      user_id: row.owner_user_id,
      source_type: row.source_type,
      source_id: row.source_id,
      target_type: row.target_type,
      target_id: row.target_id,
    }),
  normalize: normalizeMentionRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistMentionChanges,
};

export function SqliteProductivityShadow({ store }: { store: Store }) {
  return (
    <>
      <SqliteTableShadow config={TASK_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={MENTION_SHADOW_CONFIG} store={store} />
    </>
  );
}

export function normalizeTaskRow(
  row: Partial<MainTableRow<"tasks">>,
): MainTableRow<"tasks"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    task_id: typeof row.task_id === "string" ? row.task_id : "",
    source_id: typeof row.source_id === "string" ? row.source_id : "",
    source_type: typeof row.source_type === "string" ? row.source_type : "",
    source_order: typeof row.source_order === "number" ? row.source_order : 0,
    status: typeof row.status === "string" ? row.status : "todo",
    text_preview: typeof row.text_preview === "string" ? row.text_preview : "",
    body_json: typeof row.body_json === "string" ? row.body_json : "{}",
    due_date: typeof row.due_date === "string" ? row.due_date : "",
  };
}

export function normalizeMentionRow(
  row: Partial<MainTableRow<"mapping_mention">>,
): MainTableRow<"mapping_mention"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    source_id: typeof row.source_id === "string" ? row.source_id : "",
    source_type: typeof row.source_type === "string" ? row.source_type : "",
    target_id: typeof row.target_id === "string" ? row.target_id : "",
    target_type: typeof row.target_type === "string" ? row.target_type : "",
  };
}

async function persistTaskChanges(
  rows: Array<[string, MainTableRow<"tasks">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(actionItems)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeTaskRow(value);
          return {
            id,
            workspaceId: "",
            sessionId: row.source_type === "session" ? row.source_id : "",
            sourceType: row.source_type,
            sourceId: row.source_id,
            sourceOrder: row.source_order,
            assigneeHumanId: "",
            status: row.status,
            text: row.text_preview,
            bodyJson: row.body_json,
            dueAt: row.due_date,
            completedAt: null,
            createdBy: row.user_id,
            updatedBy: row.user_id,
            metadataJson: "{}",
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: actionItems.id,
        set: {
          sessionId: sql`excluded.session_id`,
          sourceType: sql`excluded.source_type`,
          sourceId: sql`excluded.source_id`,
          sourceOrder: sql`excluded.source_order`,
          status: sql`excluded.status`,
          text: sql`excluded.text`,
          bodyJson: sql`excluded.body_json`,
          dueAt: sql`excluded.due_at`,
          updatedBy: sql`excluded.updated_by`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(actionItems)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(actionItems.id, id));
  });
}

async function persistMentionChanges(
  rows: Array<[string, MainTableRow<"mapping_mention">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(entityMentions)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeMentionRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            sourceType: row.source_type,
            sourceId: row.source_id,
            targetType: row.target_type,
            targetId: row.target_id,
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: entityMentions.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          sourceType: sql`excluded.source_type`,
          sourceId: sql`excluded.source_id`,
          targetType: sql`excluded.target_type`,
          targetId: sql`excluded.target_id`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(entityMentions)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(entityMentions.id, id));
  });
}

async function softDelete(
  ids: string[],
  deleteRow: (id: string, deletedAt: string) => Promise<void>,
) {
  const deletedAt = new Date().toISOString();
  for (const id of ids) {
    await deleteRow(id, deletedAt);
  }
}

function chunks<T>(items: T[], size: number): T[][] {
  const result: T[][] = [];
  for (let index = 0; index < items.length; index += size) {
    result.push(items.slice(index, index + size));
  }
  return result;
}
