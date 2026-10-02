import { useRef } from "react";

import { commands as calendarCommands } from "@anlg/plugin-calendar";

import { trackAnalyticsEvent } from "~/analytics";
import { liveQueryClient, useLiveQuery } from "~/db";
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
  avatar_data_url: string | null;
  contact_summary_json: string | null;
};

export type ContactSummaryRecord = {
  facts: string[];
  sourceHash: string;
  promptKey: string;
  generatedAt: string;
  sources: Array<{ id: string; updatedAt: string }>;
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
  avatarDataUrl: string | null;
  summary: ContactSummaryRecord | null;
};

type HumanDisplaySqlRow = {
  id: string;
  organization_id: string;
  name: string;
  email: string;
};

export type HumanDisplayRecord = {
  id: string;
  organizationId: string;
  name: string;
  email: string;
};

type OrganizationSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  name: string;
  memo: string;
  pinned: boolean | number;
  pin_order: number | null;
  avatar_data_url: string | null;
  team_workspace: boolean | number | null;
};

export type OrganizationRecord = {
  id: string;
  userId: string;
  createdAt: string;
  name: string;
  memo: string;
  pinned: boolean;
  pinOrder: number | null;
  avatarDataUrl: string | null;
  teamWorkspace: boolean;
};

type OrganizationDisplaySqlRow = {
  id: string;
  name: string;
};

export type OrganizationDisplayRecord = {
  id: string;
  name: string;
};

const AVATAR_SQL = `CASE
  WHEN json_valid(metadata_json)
  THEN json_extract(metadata_json, '$.avatarDataUrl')
END AS avatar_data_url`;

const CONTACT_SUMMARY_SQL = `CASE
  WHEN json_valid(metadata_json)
  THEN json_extract(metadata_json, '$.contactSummary')
END AS contact_summary_json`;

const TEAM_WORKSPACE_SQL = `CASE
  WHEN json_valid(metadata_json)
  THEN COALESCE(json_extract(metadata_json, '$.teamWorkspace'), 0)
  ELSE 0
END AS team_workspace`;

type HumanSessionSqlRow = {
  id: string;
  title: string;
  created_at: string;
  source_updated_at: string;
};

export type HumanSessionRecord = {
  id: string;
  title: string;
  createdAt: string;
  sourceUpdatedAt: string;
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
const EMPTY_HUMAN_DISPLAY_RECORDS: HumanDisplayRecord[] = [];
const EMPTY_ORGANIZATION_DISPLAY_RECORDS: OrganizationDisplayRecord[] = [];
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
        pin_order,
        ${AVATAR_SQL},
        ${CONTACT_SUMMARY_SQL}
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
      SELECT id, owner_user_id, created_at, name, memo, pinned, pin_order,
        ${AVATAR_SQL},
        ${TEAM_WORKSPACE_SQL}
      FROM organizations
      WHERE deleted_at IS NULL
      ORDER BY name, id
    `,
    mapRows: (rows) => rows.map(mapOrganizationRow),
  });
  return data;
}

export function useHumanDisplayRecordsByIds(
  humanIds: readonly string[],
): HumanDisplayRecord[] {
  const uniqueIds = [...new Set(humanIds.filter(Boolean))].sort();
  const placeholders = uniqueIds.map(() => "?").join(", ");
  const enabled = uniqueIds.length > 0;
  const { data } = useLiveQuery<HumanDisplaySqlRow, HumanDisplayRecord[]>({
    sql: `
      SELECT id, organization_id, name, email
      FROM humans
      WHERE id IN (${placeholders || "NULL"})
        AND deleted_at IS NULL
      ORDER BY id
    `,
    params: uniqueIds,
    enabled,
    mapRows: (rows) =>
      rows.map((row) => ({
        id: row.id,
        organizationId: row.organization_id,
        name: row.name,
        email: row.email,
      })),
  });
  return useHeldLiveQueryRows(data, EMPTY_HUMAN_DISPLAY_RECORDS, enabled);
}

export function useOrganizationDisplayRecordsByIds(
  organizationIds: readonly string[],
): OrganizationDisplayRecord[] {
  const uniqueIds = [...new Set(organizationIds.filter(Boolean))].sort();
  const placeholders = uniqueIds.map(() => "?").join(", ");
  const enabled = uniqueIds.length > 0;
  const { data } = useLiveQuery<
    OrganizationDisplaySqlRow,
    OrganizationDisplayRecord[]
  >({
    sql: `
      SELECT id, name
      FROM organizations
      WHERE id IN (${placeholders || "NULL"})
        AND deleted_at IS NULL
      ORDER BY id
    `,
    params: uniqueIds,
    enabled,
  });
  return useHeldLiveQueryRows(
    data,
    EMPTY_ORGANIZATION_DISPLAY_RECORDS,
    enabled,
  );
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
        pin_order,
        ${AVATAR_SQL},
        ${CONTACT_SUMMARY_SQL}
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
      SELECT id, owner_user_id, created_at, name, memo, pinned, pin_order,
        ${AVATAR_SQL},
        ${TEAM_WORKSPACE_SQL}
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
      SELECT
        sessions.id,
        sessions.title,
        sessions.created_at,
        MAX(
          sessions.updated_at,
          COALESCE((
            SELECT MAX(mapping.updated_at)
            FROM session_participants AS mapping
            WHERE mapping.session_id = sessions.id
              AND mapping.human_id = ?
              AND mapping.source <> 'excluded'
              AND mapping.deleted_at IS NULL
          ), ''),
          COALESCE((
            SELECT MAX(document.updated_at)
            FROM session_documents AS document
            WHERE document.session_id = sessions.id
              AND document.kind IN ('note', 'summary', 'template_output')
              AND document.deleted_at IS NULL
          ), ''),
          COALESCE((
            SELECT MAX(transcript.updated_at)
            FROM transcripts AS transcript
            WHERE transcript.session_id = sessions.id
              AND transcript.deleted_at IS NULL
          ), '')
        ) AS source_updated_at
      FROM sessions
      WHERE sessions.deleted_at IS NULL
        AND EXISTS (
          SELECT 1
          FROM session_participants AS mapping
          WHERE mapping.session_id = sessions.id
            AND mapping.human_id = ?
            AND mapping.source <> 'excluded'
            AND mapping.deleted_at IS NULL
        )
      ORDER BY sessions.created_at DESC, sessions.id
    `,
    params: [humanId, humanId],
    mapRows: (rows) =>
      rows.map((row) => ({
        id: row.id,
        title: row.title,
        createdAt: row.created_at,
        sourceUpdatedAt: row.source_updated_at,
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
  entryPoint = "contacts",
}: {
  ownerUserId?: string;
  name: string;
  email?: string;
  entryPoint?: "contacts" | "session_participants" | "speaker_assignment";
}): Promise<string> {
  const humanId = id();

  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    const result = await calendarCommands.createHuman({
      id: humanId,
      owner_user_id: ownerUserId,
      name,
      email,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
    trackAnalyticsEvent("contact_created", {
      entry_point: entryPoint,
      has_email: Boolean(email),
    });
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

  return enqueueDatabaseWrite(`organization:${organizationId}`, async () => {
    const result = await calendarCommands.createOrganization({
      id: organizationId,
      owner_user_id: ownerUserId,
      name,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
    return organizationId;
  });
}

export function usePersonalContact(humanId: string) {
  return useLiveQuery<HumanSqlRow, HumanRecord | null>({
    sql: `SELECT *, ${AVATAR_SQL}, ${CONTACT_SUMMARY_SQL}
      FROM humans WHERE id = ? AND deleted_at IS NULL`,
    params: [humanId],
    mapRows: (rows) => (rows[0] ? mapHumanRow(rows[0]) : null),
  });
}

export function savePersonalContact(
  humanId: string,
  values: Pick<
    HumanRecord,
    | "name"
    | "email"
    | "phone"
    | "jobTitle"
    | "linkedinUsername"
    | "memo"
    | "organizationId"
  > & { avatarDataUrl?: string | null },
): Promise<void> {
  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    const result = await calendarCommands.savePersonalContact({
      human_id: humanId,
      name: values.name,
      email: values.email,
      phone: values.phone,
      job_title: values.jobTitle,
      linkedin_username: values.linkedinUsername,
      memo: values.memo,
      organization_id: values.organizationId,
      avatar_data_url:
        typeof values.avatarDataUrl === "string" ? values.avatarDataUrl : null,
      remove_avatar: values.avatarDataUrl === null,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
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
  if (Object.keys(changes).length === 0) return Promise.resolve();

  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    const result = await calendarCommands.updateHuman({
      human_id: humanId,
      name: changes.name ?? null,
      email: changes.email ?? null,
      phone: changes.phone ?? null,
      job_title: changes.jobTitle ?? null,
      linkedin_username: changes.linkedinUsername ?? null,
      memo: changes.memo ?? null,
      organization_id: changes.organizationId ?? null,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function updateOrganization(
  organizationId: string,
  changes: Partial<Pick<OrganizationRecord, "name" | "memo">>,
): Promise<void> {
  if (changes.name === undefined && changes.memo === undefined) {
    return Promise.resolve();
  }

  return enqueueDatabaseWrite(`organization:${organizationId}`, async () => {
    const result = await calendarCommands.updateOrganization({
      organization_id: organizationId,
      name: changes.name ?? null,
      memo: changes.memo ?? null,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function deleteHuman(humanId: string): Promise<void> {
  return softDeleteContact("human", humanId);
}

export function deleteOrganization(organizationId: string): Promise<void> {
  return softDeleteContact("organization", organizationId);
}

export function updateContactAvatar(
  type: "human" | "organization",
  contactId: string,
  avatarDataUrl: string | null,
): Promise<void> {
  const table = type === "human" ? "humans" : "organizations";
  return enqueueDatabaseWrite(`${table}:${contactId}`, async () => {
    const result = await calendarCommands.updateContactAvatar({
      kind: type,
      contact_id: contactId,
      avatar_data_url: avatarDataUrl,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function updateHumanContactSummary(
  humanId: string,
  summary: ContactSummaryRecord,
): Promise<void> {
  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    const result = await calendarCommands.updateHumanContactSummary({
      human_id: humanId,
      summary_json: JSON.stringify(summary),
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function toggleContactPin(
  type: "human" | "organization",
  contactId: string,
): Promise<void> {
  return enqueueDatabaseWrite("contacts:pin-order", async () => {
    const result = await calendarCommands.toggleContactPin({
      kind: type,
      contact_id: contactId,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function reorderPinnedContacts(
  contacts: Array<{ type: "human" | "organization"; id: string }>,
): Promise<void> {
  return enqueueDatabaseWrite("contacts:pin-order", async () => {
    const result = await calendarCommands.reorderPinnedContacts({
      contacts: contacts.map(({ type, id }) => ({ kind: type, id })),
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function mergeHumans(
  selectedHumanId: string,
  duplicateHumanId: string,
): Promise<void> {
  return enqueueDatabaseWrite("contacts:merge", async () => {
    const result = await calendarCommands.mergeHumans({
      selected_human_id: selectedHumanId,
      duplicate_human_id: duplicateHumanId,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
    trackAnalyticsEvent("contact_merged", {
      entry_point: "contact_details",
    });
  });
}

export function applyContactEnhancement({
  humanId,
  ownerUserId,
  changes,
  createIfMissing = false,
}: {
  humanId: string;
  ownerUserId: string;
  changes: {
    name?: string;
    email?: string;
    companyName?: string;
    jobTitle?: string;
    phone?: string;
    linkedinUsername?: string;
  };
  createIfMissing?: boolean;
}): Promise<void> {
  return enqueueDatabaseWrite(`human:${humanId}`, async () => {
    if (createIfMissing) {
      trackAnalyticsEvent("contact_created", {
        entry_point: "session_participants",
        has_email: Boolean(changes.email),
      });
    }
    const result = await calendarCommands.applyContactEnhancement({
      human_id: humanId,
      owner_user_id: ownerUserId,
      create_if_missing: createIfMissing,
      name: changes.name ?? null,
      email: changes.email ?? null,
      company_name: changes.companyName ?? null,
      job_title: changes.jobTitle ?? null,
      phone: changes.phone ?? null,
      linkedin_username: changes.linkedinUsername ?? null,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

function useHeldLiveQueryRows<T>(
  data: T[] | undefined,
  empty: T[],
  enabled: boolean,
): T[] {
  const previous = useRef(empty);
  if (data !== undefined) {
    previous.current = data;
  }
  if (!enabled) {
    previous.current = empty;
    return empty;
  }
  return data ?? previous.current;
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
    avatarDataUrl: row.avatar_data_url ?? null,
    summary: parseContactSummary(row.contact_summary_json),
  };
}

function parseContactSummary(value: string | null | undefined) {
  if (!value) return null;

  try {
    const parsed = JSON.parse(value) as Partial<ContactSummaryRecord>;
    const facts = Array.isArray(parsed.facts)
      ? parsed.facts.filter(
          (fact): fact is string => typeof fact === "string" && !!fact.trim(),
        )
      : [];
    if (
      facts.length < 3 ||
      typeof parsed.sourceHash !== "string" ||
      typeof parsed.generatedAt !== "string"
    ) {
      return null;
    }

    const sources = Array.isArray(parsed.sources)
      ? parsed.sources.filter(
          (source) =>
            typeof source?.id === "string" &&
            typeof source?.updatedAt === "string",
        )
      : [];

    return {
      facts,
      sourceHash: parsed.sourceHash,
      promptKey: typeof parsed.promptKey === "string" ? parsed.promptKey : "",
      generatedAt: parsed.generatedAt,
      sources,
    };
  } catch {
    return null;
  }
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
    avatarDataUrl: row.avatar_data_url ?? null,
    teamWorkspace: Boolean(row.team_workspace),
  };
}

function softDeleteContact(
  type: "human" | "organization",
  contactId: string,
): Promise<void> {
  const table = type === "human" ? "humans" : "organizations";
  return enqueueDatabaseWrite(`${table}:${contactId}`, async () => {
    const result = await calendarCommands.softDeleteContact({
      kind: type,
      contact_id: contactId,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}
