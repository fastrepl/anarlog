import { eq, humans, organizations, sql } from "@hypr/db";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteHumanRow = {
  id: string;
  owner_user_id: string;
  organization_id: string;
  name: string;
  email: string;
  phone: string;
  job_title: string;
  linkedin_username: string;
  memo: string;
  pinned: number | boolean;
  pin_order: number | null;
  created_at: string;
  deleted_at: string | null;
};

type SqliteOrganizationRow = {
  id: string;
  owner_user_id: string;
  name: string;
  pinned: number | boolean;
  pin_order: number | null;
  created_at: string;
  deleted_at: string | null;
};

const HUMAN_SHADOW_CONFIG: SqliteTableShadowConfig<"humans", SqliteHumanRow> = {
  label: "SqliteHumanShadow",
  tableId: "humans",
  selectSql: `
    SELECT
      id,
      owner_user_id,
      organization_id,
      name,
      email,
      phone,
      job_title,
      linkedin_username,
      memo,
      pinned,
      pin_order,
      created_at,
      deleted_at
    FROM humans
    ORDER BY name, id
  `,
  fromSqlite: (row) =>
    normalizeHumanRow({
      user_id: row.owner_user_id,
      created_at: row.created_at,
      name: row.name,
      email: row.email,
      phone: row.phone,
      org_id: row.organization_id,
      job_title: row.job_title,
      linkedin_username: row.linkedin_username,
      memo: row.memo,
      pinned: Boolean(row.pinned),
      pin_order: row.pin_order ?? undefined,
    }),
  normalize: normalizeHumanRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistHumanChanges,
};

const ORGANIZATION_SHADOW_CONFIG: SqliteTableShadowConfig<
  "organizations",
  SqliteOrganizationRow
> = {
  label: "SqliteOrganizationShadow",
  tableId: "organizations",
  selectSql: `
    SELECT id, owner_user_id, name, pinned, pin_order, created_at, deleted_at
    FROM organizations
    ORDER BY name, id
  `,
  fromSqlite: (row) =>
    normalizeOrganizationRow({
      user_id: row.owner_user_id,
      created_at: row.created_at,
      name: row.name,
      pinned: Boolean(row.pinned),
      pin_order: row.pin_order ?? undefined,
    }),
  normalize: normalizeOrganizationRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistOrganizationChanges,
};

export function SqliteContactsShadow({ store }: { store: Store }) {
  return (
    <>
      <SqliteTableShadow config={HUMAN_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={ORGANIZATION_SHADOW_CONFIG} store={store} />
    </>
  );
}

export function normalizeHumanRow(
  row: Partial<MainTableRow<"humans">>,
): MainTableRow<"humans"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : undefined,
    name: typeof row.name === "string" ? row.name : "",
    email: typeof row.email === "string" ? row.email : "",
    phone: typeof row.phone === "string" ? row.phone : "",
    org_id: typeof row.org_id === "string" ? row.org_id : "",
    job_title: typeof row.job_title === "string" ? row.job_title : "",
    linkedin_username:
      typeof row.linkedin_username === "string" ? row.linkedin_username : "",
    memo: typeof row.memo === "string" ? row.memo : "",
    pinned: typeof row.pinned === "boolean" ? row.pinned : false,
    pin_order: typeof row.pin_order === "number" ? row.pin_order : undefined,
  };
}

export function normalizeOrganizationRow(
  row: Partial<MainTableRow<"organizations">>,
): MainTableRow<"organizations"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : undefined,
    name: typeof row.name === "string" ? row.name : "",
    pinned: typeof row.pinned === "boolean" ? row.pinned : false,
    pin_order: typeof row.pin_order === "number" ? row.pin_order : undefined,
  };
}

async function persistHumanChanges(
  rows: Array<[string, MainTableRow<"humans">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(humans)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeHumanRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            organizationId: row.org_id,
            name: row.name,
            email: row.email,
            phone: row.phone,
            jobTitle: row.job_title,
            linkedinUsername: row.linkedin_username,
            memo: row.memo,
            pinned: row.pinned,
            pinOrder: row.pin_order ?? null,
            metadataJson: "{}",
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: humans.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          organizationId: sql`excluded.organization_id`,
          name: sql`excluded.name`,
          email: sql`excluded.email`,
          phone: sql`excluded.phone`,
          jobTitle: sql`excluded.job_title`,
          linkedinUsername: sql`excluded.linkedin_username`,
          memo: sql`excluded.memo`,
          pinned: sql`excluded.pinned`,
          pinOrder: sql`excluded.pin_order`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(humans)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(humans.id, id));
  });
}

async function persistOrganizationChanges(
  rows: Array<[string, MainTableRow<"organizations">]>,
  deletedIds: string[],
) {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(organizations)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeOrganizationRow(value);
          return {
            id,
            workspaceId: "",
            ownerUserId: row.user_id,
            name: row.name,
            memo: "",
            pinned: row.pinned,
            pinOrder: row.pin_order ?? null,
            metadataJson: "{}",
            createdAt: row.created_at || now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: organizations.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          name: sql`excluded.name`,
          pinned: sql`excluded.pinned`,
          pinOrder: sql`excluded.pin_order`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDelete(deletedIds, async (id, deletedAt) => {
    await db
      .update(organizations)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(organizations.id, id));
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
