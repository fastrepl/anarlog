import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

type HumanSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  organization_id: string;
  name: string;
  email: string;
  phone: string;
  job_title: string;
  linkedin_username: string;
  memo: string;
  pinned: boolean | number;
  pin_order: number | null;
};

export type HumanRecord = {
  id: string;
  userId: string;
  createdAt: string;
  organizationId: string;
  name: string;
  email: string;
  phone: string;
  jobTitle: string;
  linkedinUsername: string;
  memo: string;
  pinned: boolean;
  pinOrder: number | null;
};

const EMPTY_HUMANS: HumanRecord[] = [];

export function useHumans(): HumanRecord[] {
  const { data = EMPTY_HUMANS } = useLiveQuery<HumanSqlRow, HumanRecord[]>({
    sql: `
      SELECT
        id,
        owner_user_id,
        created_at,
        organization_id,
        name,
        email,
        phone,
        job_title,
        linkedin_username,
        memo,
        pinned,
        pin_order
      FROM humans
      WHERE deleted_at IS NULL
      ORDER BY name, email, id
    `,
    mapRows: (rows) => rows.map(mapHumanRow),
  });
  return data;
}

export function createHuman({
  ownerUserId,
  name,
  email = "",
}: {
  ownerUserId: string;
  name: string;
  email?: string;
}): Promise<string> {
  const humanId = id();
  const now = new Date().toISOString();

  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    await executeTransaction([
      {
        sql: `
          INSERT INTO humans (
            id, workspace_id, owner_user_id, organization_id, name, email,
            phone, job_title, linkedin_username, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (?, '', ?, '', ?, ?, '', '', '', '', 0, NULL, '{}', ?, ?, NULL)
        `,
        params: [humanId, ownerUserId, name, email, now, now],
      },
    ]);
    return humanId;
  });
}

export function applyContactEnhancement({
  humanId,
  ownerUserId,
  changes,
}: {
  humanId: string;
  ownerUserId: string;
  changes: { name?: string; email?: string; companyName?: string };
}): Promise<void> {
  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    const now = new Date().toISOString();
    const statements: Array<{ sql: string; params: unknown[] }> = [];

    if (changes.companyName) {
      const organizationId = id();
      statements.push({
        sql: `
          INSERT INTO organizations (
            id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          )
          SELECT ?, '', ?, ?, '', 0, NULL, '{}', ?, ?, NULL
          WHERE NOT EXISTS (
            SELECT 1
            FROM organizations
            WHERE lower(name) = lower(?) AND deleted_at IS NULL
          )
        `,
        params: [
          organizationId,
          ownerUserId,
          changes.companyName,
          now,
          now,
          changes.companyName,
        ],
      });
    }

    const assignments: string[] = [];
    const params: unknown[] = [];
    if (changes.name !== undefined) {
      assignments.push("name = ?");
      params.push(changes.name);
    }
    if (changes.email !== undefined) {
      assignments.push("email = ?");
      params.push(changes.email);
    }
    if (changes.companyName) {
      assignments.push(`
        organization_id = CASE
          WHEN organization_id = '' THEN COALESCE((
            SELECT id
            FROM organizations
            WHERE lower(name) = lower(?) AND deleted_at IS NULL
            ORDER BY created_at, id
            LIMIT 1
          ), organization_id)
          ELSE organization_id
        END
      `);
      params.push(changes.companyName);
    }

    if (assignments.length > 0) {
      statements.push({
        sql: `
          UPDATE humans
          SET ${assignments.join(", ")}, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [...params, now, humanId],
      });
    }

    if (statements.length > 0) await executeTransaction(statements);
  });
}

function mapHumanRow(row: HumanSqlRow): HumanRecord {
  return {
    id: row.id,
    userId: row.owner_user_id,
    createdAt: row.created_at,
    organizationId: row.organization_id,
    name: row.name,
    email: row.email,
    phone: row.phone,
    jobTitle: row.job_title,
    linkedinUsername: row.linkedin_username,
    memo: row.memo,
    pinned: Boolean(row.pinned),
    pinOrder: row.pin_order,
  };
}
