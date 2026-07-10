import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { DEFAULT_USER_ID, id } from "~/shared/utils";

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

type OrganizationSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  name: string;
  memo: string;
  pinned: boolean | number;
  pin_order: number | null;
};

export type OrganizationRecord = {
  id: string;
  userId: string;
  createdAt: string;
  name: string;
  memo: string;
  pinned: boolean;
  pinOrder: number | null;
};

type HumanSessionSqlRow = {
  id: string;
  title: string;
  created_at: string;
};

export type HumanSessionRecord = {
  id: string;
  title: string;
  createdAt: string;
};

type ContactSearchSqlRow = {
  id: string;
  name: string;
  email: string;
  phone: string;
  job_title: string;
  organization_name: string;
  memo: string;
};

export type ContactSearchRecord = {
  id: string;
  name: string;
  email: string | null;
  phone: string | null;
  jobTitle: string | null;
  organization: string | null;
  memo: string | null;
};

const EMPTY_HUMANS: HumanRecord[] = [];
const EMPTY_ORGANIZATIONS: OrganizationRecord[] = [];
const EMPTY_HUMAN_SESSIONS: HumanSessionRecord[] = [];

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

export function useOrganizations(): OrganizationRecord[] {
  const { data = EMPTY_ORGANIZATIONS } = useLiveQuery<
    OrganizationSqlRow,
    OrganizationRecord[]
  >({
    sql: `
      SELECT id, owner_user_id, created_at, name, memo, pinned, pin_order
      FROM organizations
      WHERE deleted_at IS NULL
      ORDER BY name, id
    `,
    mapRows: (rows) => rows.map(mapOrganizationRow),
  });
  return data;
}

export async function loadHuman(humanId: string): Promise<HumanRecord | null> {
  if (!humanId) return null;
  const rows = await loadHumansByIds([humanId]);
  return rows[0] ?? null;
}

export async function loadHumansByIds(
  humanIds: readonly string[],
): Promise<HumanRecord[]> {
  const uniqueIds = [...new Set(humanIds.filter(Boolean))].sort();
  if (uniqueIds.length === 0) return [];

  const rows = await liveQueryClient.execute<HumanSqlRow>(
    `
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
      WHERE id IN (${uniqueIds.map(() => "?").join(", ")})
        AND deleted_at IS NULL
      ORDER BY id
    `,
    uniqueIds,
  );
  return rows.map(mapHumanRow);
}

export async function loadOrganization(
  organizationId: string,
): Promise<OrganizationRecord | null> {
  if (!organizationId) return null;
  const rows = await liveQueryClient.execute<OrganizationSqlRow>(
    `
      SELECT id, owner_user_id, created_at, name, memo, pinned, pin_order
      FROM organizations
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [organizationId],
  );
  return rows[0] ? mapOrganizationRow(rows[0]) : null;
}

export function useHumanSessions(humanId: string): HumanSessionRecord[] {
  const { data = EMPTY_HUMAN_SESSIONS } = useLiveQuery<
    HumanSessionSqlRow,
    HumanSessionRecord[]
  >({
    sql: `
      SELECT DISTINCT sessions.id, sessions.title, sessions.created_at
      FROM session_participants
      INNER JOIN sessions ON sessions.id = session_participants.session_id
      WHERE session_participants.human_id = ?
        AND session_participants.source <> 'excluded'
        AND session_participants.deleted_at IS NULL
        AND sessions.deleted_at IS NULL
      ORDER BY sessions.created_at DESC, sessions.id
    `,
    params: [humanId],
    mapRows: (rows) =>
      rows.map((row) => ({
        id: row.id,
        title: row.title,
        createdAt: row.created_at,
      })),
  });
  return data;
}

export async function searchContacts(
  query: string,
  limit: number,
): Promise<ContactSearchRecord[]> {
  const normalizedQuery = query.trim().toLowerCase();
  const rows = await liveQueryClient.execute<ContactSearchSqlRow>(
    `
      SELECT
        humans.id,
        humans.name,
        humans.email,
        humans.phone,
        humans.job_title,
        COALESCE(organizations.name, '') AS organization_name,
        humans.memo
      FROM humans
      LEFT JOIN organizations
        ON organizations.id = humans.organization_id
        AND organizations.deleted_at IS NULL
      WHERE humans.deleted_at IS NULL
        AND (
          ? = '' OR lower(
            humans.name || char(10) ||
            humans.email || char(10) ||
            humans.phone || char(10) ||
            humans.job_title || char(10) ||
            humans.memo || char(10) ||
            COALESCE(organizations.name, '')
          ) LIKE '%' || ? || '%'
        )
      ORDER BY humans.created_at DESC, humans.id
      LIMIT ?
    `,
    [normalizedQuery, normalizedQuery, limit],
  );
  return rows.map((row) => ({
    id: row.id,
    name: row.name,
    email: row.email || null,
    phone: row.phone || null,
    jobTitle: row.job_title || null,
    organization: row.organization_name || null,
    memo: row.memo || null,
  }));
}

export function createHuman({
  ownerUserId = DEFAULT_USER_ID,
  name,
  email = "",
}: {
  ownerUserId?: string;
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

export function createOrganization({
  ownerUserId = DEFAULT_USER_ID,
  name,
}: {
  ownerUserId?: string;
  name: string;
}): Promise<string> {
  const organizationId = id();
  const now = new Date().toISOString();

  return enqueueDatabaseWrite(`organization:${organizationId}`, async () => {
    await executeTransaction([
      {
        sql: `
          INSERT INTO organizations (
            id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (?, '', ?, ?, '', 0, NULL, '{}', ?, ?, NULL)
        `,
        params: [organizationId, ownerUserId, name, now, now],
      },
    ]);
    return organizationId;
  });
}

export function updateHuman(
  humanId: string,
  changes: Partial<
    Pick<
      HumanRecord,
      | "name"
      | "email"
      | "phone"
      | "jobTitle"
      | "linkedinUsername"
      | "memo"
      | "organizationId"
    >
  >,
): Promise<void> {
  const columns = {
    name: "name",
    email: "email",
    phone: "phone",
    jobTitle: "job_title",
    linkedinUsername: "linkedin_username",
    memo: "memo",
    organizationId: "organization_id",
  } as const;
  const assignments: string[] = [];
  const params: unknown[] = [];

  for (const [key, value] of Object.entries(changes) as Array<
    [keyof typeof columns, string]
  >) {
    assignments.push(`${columns[key]} = ?`);
    params.push(value);
  }
  if (assignments.length === 0) return Promise.resolve();

  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    await executeTransaction([
      {
        sql: `
          UPDATE humans
          SET ${assignments.join(", ")}, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [...params, new Date().toISOString(), humanId],
      },
    ]);
  });
}

export function updateOrganization(
  organizationId: string,
  changes: Partial<Pick<OrganizationRecord, "name" | "memo">>,
): Promise<void> {
  const assignments: string[] = [];
  const params: unknown[] = [];
  if (changes.name !== undefined) {
    assignments.push("name = ?");
    params.push(changes.name);
  }
  if (changes.memo !== undefined) {
    assignments.push("memo = ?");
    params.push(changes.memo);
  }
  if (assignments.length === 0) return Promise.resolve();

  return enqueueDatabaseWrite(`organization:${organizationId}`, async () => {
    await executeTransaction([
      {
        sql: `
          UPDATE organizations
          SET ${assignments.join(", ")}, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [...params, new Date().toISOString(), organizationId],
      },
    ]);
  });
}

export function deleteHuman(humanId: string): Promise<void> {
  return softDeleteContact("humans", humanId);
}

export function deleteOrganization(organizationId: string): Promise<void> {
  return softDeleteContact("organizations", organizationId);
}

export function toggleContactPin(
  type: "human" | "organization",
  contactId: string,
): Promise<void> {
  const table = type === "human" ? "humans" : "organizations";
  return enqueueDatabaseWrite("contacts:pin-order", async () => {
    await executeTransaction([
      {
        sql: `
          UPDATE ${table}
          SET
            pin_order = CASE
              WHEN pinned = 1 THEN NULL
              ELSE COALESCE((
                SELECT MAX(pin_order)
                FROM (
                  SELECT pin_order FROM humans WHERE deleted_at IS NULL
                  UNION ALL
                  SELECT pin_order FROM organizations WHERE deleted_at IS NULL
                )
              ), 0) + 1
            END,
            pinned = CASE WHEN pinned = 1 THEN 0 ELSE 1 END,
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [new Date().toISOString(), contactId],
      },
    ]);
  });
}

export function reorderPinnedContacts(
  contacts: Array<{ type: "human" | "organization"; id: string }>,
): Promise<void> {
  return enqueueDatabaseWrite("contacts:pin-order", async () => {
    const now = new Date().toISOString();
    await executeTransaction(
      contacts.map((contact, index) => ({
        sql: `
          UPDATE ${contact.type === "human" ? "humans" : "organizations"}
          SET pin_order = ?, updated_at = ?
          WHERE id = ? AND pinned = 1 AND deleted_at IS NULL
        `,
        params: [index, now, contact.id],
      })),
    );
  });
}

export function mergeHumans(
  selectedHumanId: string,
  duplicateHumanId: string,
): Promise<void> {
  return enqueueDatabaseWrite("contacts:merge", async () => {
    const primaryId =
      duplicateHumanId === DEFAULT_USER_ID ? duplicateHumanId : selectedHumanId;
    const duplicateId =
      duplicateHumanId === DEFAULT_USER_ID ? selectedHumanId : duplicateHumanId;
    const rows = await liveQueryClient.execute<HumanSqlRow>(
      `
        SELECT
          id, owner_user_id, created_at, organization_id, name, email, phone,
          job_title, linkedin_username, memo, pinned, pin_order
        FROM humans
        WHERE id IN (?, ?) AND deleted_at IS NULL
      `,
      [primaryId, duplicateId],
    );
    const primary = rows.find((row) => row.id === primaryId);
    const duplicate = rows.find((row) => row.id === duplicateId);
    if (!primary || !duplicate) {
      throw new Error("Both contacts must exist before they can be merged");
    }

    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants AS duplicate_mapping
          SET deleted_at = ?, updated_at = ?
          WHERE duplicate_mapping.human_id = ?
            AND duplicate_mapping.deleted_at IS NULL
            AND EXISTS (
              SELECT 1
              FROM session_participants AS primary_mapping
              WHERE primary_mapping.session_id = duplicate_mapping.session_id
                AND primary_mapping.human_id = ?
                AND primary_mapping.deleted_at IS NULL
            )
        `,
        params: [now, now, duplicateId, primaryId],
      },
      {
        sql: `
          UPDATE session_participants
          SET human_id = ?, updated_at = ?
          WHERE human_id = ? AND deleted_at IS NULL
        `,
        params: [primaryId, now, duplicateId],
      },
      {
        sql: `
          UPDATE humans
          SET
            job_title = ?,
            linkedin_username = ?,
            phone = ?,
            memo = ?,
            organization_id = ?,
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [
          mergeText(primary.job_title, duplicate.job_title),
          mergeText(primary.linkedin_username, duplicate.linkedin_username),
          mergeText(primary.phone, duplicate.phone),
          mergeText(primary.memo, duplicate.memo),
          primary.organization_id || duplicate.organization_id,
          now,
          primaryId,
        ],
      },
      {
        sql: `
          UPDATE humans
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, duplicateId],
      },
    ]);
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

function mapOrganizationRow(row: OrganizationSqlRow): OrganizationRecord {
  return {
    id: row.id,
    userId: row.owner_user_id,
    createdAt: row.created_at,
    name: row.name,
    memo: row.memo,
    pinned: Boolean(row.pinned),
    pinOrder: row.pin_order,
  };
}

function softDeleteContact(
  table: "humans" | "organizations",
  contactId: string,
): Promise<void> {
  return enqueueDatabaseWrite(`${table}:${contactId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE ${table}
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, contactId],
      },
    ]);
  });
}

function mergeText(primary: string, duplicate: string): string {
  if (!duplicate) return primary;
  return primary ? `${primary}, ${duplicate}` : duplicate;
}
