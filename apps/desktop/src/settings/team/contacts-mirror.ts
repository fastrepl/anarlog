import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { DEFAULT_USER_ID } from "~/shared/utils";

export type WorkspaceContactMember = {
  userId: string;
  email: string;
  name: string | null;
  avatarUrl: string | null;
};

export type WorkspaceContactMirror = {
  workspaceId: string;
  name: string;
  logoDataUrl: string | null;
  // undefined when the roster could not be listed: members are left untouched.
  members?: WorkspaceContactMember[];
};

const BINDING_WORKSPACE_SQL = `NULLIF((
  SELECT json_extract(value_json, '$.workspace_id')
  FROM app_settings
  WHERE id = 'cloudsync_workspace_binding'
), '')`;

// workspace_id is NOT NULL: the binding may not exist outside CloudSync.
const WORKSPACE_ID_SQL = `COALESCE(${BINDING_WORKSPACE_SQL}, '')`;

const OWNER_SQL = `COALESCE(
  (SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
  NULLIF(NULLIF(?, ''), '${DEFAULT_USER_ID}'),
  ${BINDING_WORKSPACE_SQL},
  '${DEFAULT_USER_ID}'
)`;

type OrganizationStateRow = {
  id: string;
  name: string;
  metadata_json: string;
  deleted_at: string | null;
};

type HumanStateRow = {
  id: string;
  name: string;
  email: string;
  organization_id: string;
  metadata_json: string;
  deleted_at: string | null;
};

function parseMetadata(raw: string): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (
      parsed !== null &&
      typeof parsed === "object" &&
      !Array.isArray(parsed)
    ) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    // Malformed metadata is treated as empty rather than blocking the mirror.
  }
  return {};
}

const text = (value: unknown): string | null =>
  typeof value === "string" && value !== "" ? value : null;

function metadataMatches(
  metadata: Record<string, unknown>,
  expected: Record<string, unknown>,
): boolean {
  return Object.entries(expected).every(
    ([key, value]) => metadata[key] === value,
  );
}

/**
 * Mirrors shared workspaces and their rosters into Contacts.
 *
 * Each workspace becomes an organizations row keyed by the workspace id whose
 * name and logo stay in sync until the user edits them, the same way the
 * personal contact card tracks the account profile. Members become humans
 * rows keyed by their user id and linked to the workspace's organization.
 *
 * Merge rules never overwrite contact data the user entered: a field is only
 * updated while it is empty or still holds the value the mirror last wrote
 * (tracked in metadata_json). Rows are written through the same per-contact
 * write queue keys the contact editors use so a mirror refresh cannot race a
 * manual edit.
 */
export async function mirrorWorkspaceContacts(
  workspaces: WorkspaceContactMirror[],
): Promise<void> {
  const keptIds = workspaces.map((workspace) => workspace.workspaceId);

  for (const workspace of workspaces) {
    await enqueueDatabaseWrite(`organization:${workspace.workspaceId}`, () =>
      upsertWorkspaceOrganization(workspace),
    );

    if (workspace.members) {
      for (const member of workspace.members) {
        await enqueueDatabaseWrite(`human:${member.userId}`, () =>
          upsertWorkspaceMember(workspace, member),
        );
      }
      await unlinkDepartedMembers(workspace);
    }
  }

  await unlinkStaleWorkspaceContacts(keptIds);
}

async function upsertWorkspaceOrganization(
  workspace: WorkspaceContactMirror,
): Promise<void> {
  const existing = await liveQueryClient.execute<OrganizationStateRow>(
    `SELECT id, name, metadata_json, deleted_at FROM organizations WHERE id = ?`,
    [workspace.workspaceId],
  );
  const row = existing[0];
  const now = new Date().toISOString();

  if (!row) {
    await executeTransaction([
      {
        sql: `
          INSERT INTO organizations (
            id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (?, ${WORKSPACE_ID_SQL}, ${OWNER_SQL}, ?, '', 0, NULL,
            ?, ?, ?, NULL)
        `,
        params: [
          workspace.workspaceId,
          DEFAULT_USER_ID,
          workspace.name,
          JSON.stringify({
            teamWorkspace: true,
            teamName: workspace.name,
            teamLogoDataUrl: workspace.logoDataUrl,
            avatarDataUrl: workspace.logoDataUrl,
          }),
          now,
          now,
        ],
      },
    ]);
    return;
  }

  // A contact the user deleted stays deleted.
  if (row.deleted_at !== null) return;

  const metadata = parseMetadata(row.metadata_json);
  const trackedName = text(metadata.teamName);
  const trackedLogo = text(metadata.teamLogoDataUrl);
  const avatarValue = text(metadata.avatarDataUrl);
  const untouchedName =
    row.name === "" ||
    (metadata.teamWorkspace === true && row.name === trackedName);
  const untouchedAvatar =
    avatarValue === trackedLogo ||
    (avatarValue === null && trackedLogo === null);
  const nextName = untouchedName ? workspace.name : row.name;
  const nextMetadata = {
    ...metadata,
    teamWorkspace: true,
    teamName: workspace.name,
    teamLogoDataUrl: workspace.logoDataUrl,
    avatarDataUrl: untouchedAvatar ? workspace.logoDataUrl : avatarValue,
  };

  if (row.name === nextName && metadataMatches(metadata, nextMetadata)) {
    return;
  }

  await executeTransaction([
    {
      sql: `
        UPDATE organizations
        SET name = ?, metadata_json = ?, updated_at = ?
        WHERE id = ?
      `,
      params: [
        nextName,
        JSON.stringify(nextMetadata),
        now,
        workspace.workspaceId,
      ],
    },
  ]);
}

async function upsertWorkspaceMember(
  workspace: WorkspaceContactMirror,
  member: WorkspaceContactMember,
): Promise<void> {
  const existing = await liveQueryClient.execute<HumanStateRow>(
    `SELECT id, name, email, organization_id, metadata_json, deleted_at
     FROM humans WHERE id = ?`,
    [member.userId],
  );
  const row = existing[0];
  const now = new Date().toISOString();

  if (!row) {
    await executeTransaction([
      {
        sql: `
          INSERT INTO humans (
            id, workspace_id, owner_user_id, organization_id, name, email,
            phone, job_title, linkedin_username, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (?, ${WORKSPACE_ID_SQL}, ${OWNER_SQL}, ?, ?, ?,
            '', '', '', '', 0, NULL, ?, ?, ?, NULL)
        `,
        params: [
          member.userId,
          member.userId,
          workspace.workspaceId,
          member.name ?? "",
          member.email,
          JSON.stringify({
            teamWorkspaceId: workspace.workspaceId,
            teamAvatarUrl: member.avatarUrl,
            avatarDataUrl: member.avatarUrl,
          }),
          now,
          now,
        ],
      },
    ]);
    return;
  }

  // A contact the user deleted stays deleted.
  if (row.deleted_at !== null) return;

  const metadata = parseMetadata(row.metadata_json);
  const trackedWorkspace = text(metadata.teamWorkspaceId);
  const avatarValue = text(metadata.avatarDataUrl);
  const trackedAvatar = text(metadata.teamAvatarUrl);

  // An empty link only gets filled when the mirror never linked this
  // contact: clearing a team-managed company stays cleared.
  const organizationId =
    row.organization_id === trackedWorkspace ||
    (row.organization_id === "" && trackedWorkspace === null)
      ? workspace.workspaceId
      : row.organization_id;
  const untouchedAvatar =
    avatarValue === trackedAvatar ||
    (avatarValue === null && trackedAvatar === null);
  const nextMetadata = {
    ...metadata,
    teamWorkspaceId: workspace.workspaceId,
    teamAvatarUrl: member.avatarUrl,
    avatarDataUrl: untouchedAvatar ? member.avatarUrl : avatarValue,
  };
  const nextName = row.name === "" ? (member.name ?? "") : row.name;
  const nextEmail = row.email === "" ? member.email : row.email;

  if (
    row.organization_id === organizationId &&
    row.name === nextName &&
    row.email === nextEmail &&
    metadataMatches(metadata, nextMetadata)
  ) {
    return;
  }

  await executeTransaction([
    {
      sql: `
        UPDATE humans
        SET organization_id = ?, name = ?, email = ?, metadata_json = ?,
          updated_at = ?
        WHERE id = ?
      `,
      params: [
        organizationId,
        nextName,
        nextEmail,
        JSON.stringify(nextMetadata),
        now,
        member.userId,
      ],
    },
  ]);
}

/**
 * Members who left the roster keep their contact, but the link to the
 * workspace organization is dropped so the org's People list matches the
 * current team.
 */
async function unlinkDepartedMembers(
  workspace: WorkspaceContactMirror,
): Promise<void> {
  const memberIds = (workspace.members ?? []).map((member) => member.userId);
  const placeholders = memberIds.map(() => "?").join(", ") || "NULL";
  const stale = await liveQueryClient.execute<HumanStateRow>(
    `SELECT id, name, email, organization_id, metadata_json, deleted_at
     FROM humans
     WHERE deleted_at IS NULL
       AND json_valid(metadata_json)
       AND json_extract(metadata_json, '$.teamWorkspaceId') = ?
       AND id NOT IN (${placeholders})`,
    [workspace.workspaceId, ...memberIds],
  );

  for (const row of stale) {
    await enqueueDatabaseWrite(`human:${row.id}`, () =>
      unlinkWorkspaceContact(row.id),
    );
  }
}

async function unlinkStaleWorkspaceContacts(keptIds: string[]): Promise<void> {
  const placeholders = keptIds.map(() => "?").join(", ") || "NULL";
  const stale = await liveQueryClient.execute<HumanStateRow>(
    `SELECT id, name, email, organization_id, metadata_json, deleted_at
     FROM humans
     WHERE deleted_at IS NULL
       AND json_valid(metadata_json)
       AND json_extract(metadata_json, '$.teamWorkspaceId') IS NOT NULL
       AND json_extract(metadata_json, '$.teamWorkspaceId') NOT IN (${placeholders})`,
    keptIds,
  );

  for (const row of stale) {
    await enqueueDatabaseWrite(`human:${row.id}`, () =>
      unlinkWorkspaceContact(row.id),
    );
  }
}

async function unlinkWorkspaceContact(humanId: string): Promise<void> {
  const existing = await liveQueryClient.execute<HumanStateRow>(
    `SELECT id, name, email, organization_id, metadata_json, deleted_at
     FROM humans WHERE id = ?`,
    [humanId],
  );
  const row = existing[0];
  if (!row || row.deleted_at !== null) return;

  const metadata = parseMetadata(row.metadata_json);
  const trackedWorkspace = text(metadata.teamWorkspaceId);
  if (trackedWorkspace === null) return;

  delete metadata.teamWorkspaceId;
  delete metadata.teamAvatarUrl;
  const organizationId =
    row.organization_id === trackedWorkspace ? "" : row.organization_id;

  await executeTransaction([
    {
      sql: `
        UPDATE humans
        SET organization_id = ?, metadata_json = ?, updated_at = ?
        WHERE id = ?
      `,
      params: [
        organizationId,
        JSON.stringify(metadata),
        new Date().toISOString(),
        humanId,
      ],
    },
  ]);
}
