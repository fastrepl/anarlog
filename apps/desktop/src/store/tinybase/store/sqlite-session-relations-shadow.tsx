import { eq, sessionParticipants, sessionTags, sql, tags } from "@hypr/db";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteParticipantRow = {
  id: string;
  owner_user_id: string;
  session_id: string;
  human_id: string;
  source: string;
  deleted_at: string | null;
};

type SqliteTagRow = {
  id: string;
  owner_user_id: string;
  name: string;
  deleted_at: string | null;
};

type SqliteSessionTagRow = {
  id: string;
  owner_user_id: string;
  session_id: string;
  tag_id: string;
  deleted_at: string | null;
};

const PARTICIPANT_SHADOW_CONFIG: SqliteTableShadowConfig<
  "mapping_session_participant",
  SqliteParticipantRow
> = {
  label: "SqliteSessionParticipantShadow",
  tableId: "mapping_session_participant",
  selectSql: `
    SELECT id, owner_user_id, session_id, human_id, source, deleted_at
    FROM session_participants
    ORDER BY session_id, id
  `,
  fromSqlite: (row) =>
    normalizeParticipantRow({
      user_id: row.owner_user_id,
      session_id: row.session_id,
      human_id: row.human_id,
      source: row.source,
    }),
  normalize: normalizeParticipantRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistParticipantChanges,
};

const TAG_SHADOW_CONFIG: SqliteTableShadowConfig<"tags", SqliteTagRow> = {
  label: "SqliteTagShadow",
  tableId: "tags",
  selectSql: `
    SELECT id, owner_user_id, name, deleted_at
    FROM tags
    ORDER BY name, id
  `,
  fromSqlite: (row) =>
    normalizeTagRow({ user_id: row.owner_user_id, name: row.name }),
  normalize: normalizeTagRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistTagChanges,
};

const SESSION_TAG_SHADOW_CONFIG: SqliteTableShadowConfig<
  "mapping_tag_session",
  SqliteSessionTagRow
> = {
  label: "SqliteSessionTagShadow",
  tableId: "mapping_tag_session",
  selectSql: `
    SELECT id, owner_user_id, session_id, tag_id, deleted_at
    FROM session_tags
    ORDER BY session_id, tag_id, id
  `,
  fromSqlite: (row) =>
    normalizeSessionTagRow({
      user_id: row.owner_user_id,
      session_id: row.session_id,
      tag_id: row.tag_id,
    }),
  normalize: normalizeSessionTagRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistSessionTagChanges,
};

export function SqliteSessionRelationsShadow({ store }: { store: Store }) {
  return (
    <>
      <SqliteTableShadow config={PARTICIPANT_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={TAG_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={SESSION_TAG_SHADOW_CONFIG} store={store} />
    </>
  );
}

function normalizeParticipantRow(
  row: Partial<MainTableRow<"mapping_session_participant">>,
): MainTableRow<"mapping_session_participant"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    session_id: typeof row.session_id === "string" ? row.session_id : "",
    human_id: typeof row.human_id === "string" ? row.human_id : "",
    source: typeof row.source === "string" ? row.source : "",
  };
}

function normalizeTagRow(
  row: Partial<MainTableRow<"tags">>,
): MainTableRow<"tags"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    name: typeof row.name === "string" ? row.name : "",
  };
}

function normalizeSessionTagRow(
  row: Partial<MainTableRow<"mapping_tag_session">>,
): MainTableRow<"mapping_tag_session"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    session_id: typeof row.session_id === "string" ? row.session_id : "",
    tag_id: typeof row.tag_id === "string" ? row.tag_id : "",
  };
}

async function persistParticipantChanges(
  rows: Array<[string, MainTableRow<"mapping_session_participant">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(sessionParticipants)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeParticipantRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            sessionId: row.session_id,
            humanId: row.human_id,
            displayName: "",
            email: "",
            role: "",
            source: row.source,
            metadataJson: "{}",
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: sessionParticipants.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          sessionId: sql`excluded.session_id`,
          humanId: sql`excluded.human_id`,
          source: sql`excluded.source`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(sessionParticipants)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(sessionParticipants.id, id));
  });
}

async function persistTagChanges(
  rows: Array<[string, MainTableRow<"tags">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(tags)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeTagRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            name: row.name,
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: tags.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          name: sql`excluded.name`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(tags)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(tags.id, id));
  });
}

async function persistSessionTagChanges(
  rows: Array<[string, MainTableRow<"mapping_tag_session">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(sessionTags)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeSessionTagRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            sessionId: row.session_id,
            tagId: row.tag_id,
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: sessionTags.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          sessionId: sql`excluded.session_id`,
          tagId: sql`excluded.tag_id`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(sessionTags)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(sessionTags.id, id));
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
