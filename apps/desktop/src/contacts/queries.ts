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
