import { chatGroups, chatMessages, eq, sql } from "@hypr/db";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteChatGroupRow = {
  id: string;
  owner_user_id: string;
  title: string;
  created_at: string;
  deleted_at: string | null;
};

type SqliteChatMessageRow = {
  id: string;
  owner_user_id: string;
  chat_group_id: string;
  role: string;
  content: string;
  metadata_json: string;
  parts_json: string;
  status: string;
  created_at: string;
  deleted_at: string | null;
};

const CHAT_GROUP_SHADOW_CONFIG: SqliteTableShadowConfig<
  "chat_groups",
  SqliteChatGroupRow
> = {
  label: "SqliteChatGroupShadow",
  tableId: "chat_groups",
  selectSql: `
    SELECT id, owner_user_id, title, created_at, deleted_at
    FROM chat_groups
    ORDER BY created_at, id
  `,
  fromSqlite: (row) =>
    normalizeChatGroupRow({
      user_id: row.owner_user_id,
      created_at: row.created_at,
      title: row.title,
    }),
  normalize: normalizeChatGroupRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistChatGroupChanges,
};

const CHAT_MESSAGE_SHADOW_CONFIG: SqliteTableShadowConfig<
  "chat_messages",
  SqliteChatMessageRow
> = {
  label: "SqliteChatMessageShadow",
  tableId: "chat_messages",
  selectSql: `
    SELECT
      id,
      owner_user_id,
      chat_group_id,
      role,
      content,
      metadata_json,
      parts_json,
      status,
      created_at,
      deleted_at
    FROM chat_messages
    ORDER BY chat_group_id, created_at, id
  `,
  fromSqlite: (row) =>
    normalizeChatMessageRow({
      user_id: row.owner_user_id,
      created_at: row.created_at,
      chat_group_id: row.chat_group_id,
      role: row.role,
      content: row.content,
      metadata: row.metadata_json,
      parts: row.parts_json,
      status: row.status,
    }),
  normalize: normalizeChatMessageRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistChatMessageChanges,
};

export function SqliteChatShadow({ store }: { store: Store }) {
  return (
    <>
      <SqliteTableShadow config={CHAT_GROUP_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={CHAT_MESSAGE_SHADOW_CONFIG} store={store} />
    </>
  );
}

export function normalizeChatGroupRow(
  row: Partial<MainTableRow<"chat_groups">>,
): MainTableRow<"chat_groups"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    title: typeof row.title === "string" ? row.title : "",
  };
}

export function normalizeChatMessageRow(
  row: Partial<MainTableRow<"chat_messages">>,
): MainTableRow<"chat_messages"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    chat_group_id:
      typeof row.chat_group_id === "string" ? row.chat_group_id : "",
    role: typeof row.role === "string" ? row.role : "",
    content: typeof row.content === "string" ? row.content : "",
    metadata: typeof row.metadata === "string" ? row.metadata : "{}",
    parts: typeof row.parts === "string" ? row.parts : "[]",
    status: typeof row.status === "string" ? row.status : "ready",
  };
}

async function persistChatGroupChanges(
  rows: Array<[string, MainTableRow<"chat_groups">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(chatGroups)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeChatGroupRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            title: row.title,
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: chatGroups.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          title: sql`excluded.title`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(chatGroups)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(chatGroups.id, id));
  });
}

async function persistChatMessageChanges(
  rows: Array<[string, MainTableRow<"chat_messages">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(chatMessages)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeChatMessageRow(value);
          return {
            id,
            workspaceId: "",
            chatGroupId: row.chat_group_id,
            ownerUserId: row.user_id,
            role: row.role,
            content: row.content,
            metadataJson: row.metadata,
            partsJson: row.parts,
            status: row.status,
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: chatMessages.id,
        set: {
          chatGroupId: sql`excluded.chat_group_id`,
          ownerUserId: sql`excluded.owner_user_id`,
          role: sql`excluded.role`,
          content: sql`excluded.content`,
          metadataJson: sql`excluded.metadata_json`,
          partsJson: sql`excluded.parts_json`,
          status: sql`excluded.status`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(chatMessages)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(chatMessages.id, id));
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
