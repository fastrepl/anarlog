import { TeamError, type TeamContext, type WorkspaceRole } from "./client";

import { executeTransaction } from "~/db";

export type MirroredWorkspace = {
  workspaceId: string;
  name: string;
  ownerUserId: string;
  role: WorkspaceRole;
};

/**
 * Copies the shared workspaces this account belongs to into local SQLite.
 *
 * The share panel reads workspaces from the local mirror, not from the network,
 * so without this the "Everyone in {workspace}" scope never appears. Workspaces
 * that disappear server-side are tombstoned rather than deleted so an outdated
 * row can never keep granting a scope.
 */
export async function mirrorSharedWorkspaces(
  context: TeamContext,
  workspaces: MirroredWorkspace[],
): Promise<void> {
  const userId = context.session.user.id;
  if (!userId) throw new TeamError();

  const now = new Date().toISOString();
  const keptIds = workspaces.map((workspace) => workspace.workspaceId);
  const placeholders = keptIds.map(() => "?").join(",");

  const statements: {
    sql: string;
    params: (string | number | null)[];
  }[] = [];

  for (const workspace of workspaces) {
    statements.push({
      sql: `
        INSERT INTO workspaces (
          id, owner_user_id, kind, name, created_at, updated_at, deleted_at
        ) VALUES (?, ?, 'shared', ?, ?, ?, NULL)
        ON CONFLICT(id) DO UPDATE SET
          owner_user_id = excluded.owner_user_id,
          name = excluded.name,
          updated_at = excluded.updated_at,
          deleted_at = NULL
      `,
      params: [
        workspace.workspaceId,
        workspace.ownerUserId,
        workspace.name,
        now,
        now,
      ],
    });

    statements.push({
      sql: `
        INSERT INTO workspace_memberships (
          id, workspace_id, user_id, role, created_at, updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL)
        ON CONFLICT(workspace_id, user_id) DO UPDATE SET
          role = excluded.role,
          updated_at = excluded.updated_at,
          deleted_at = NULL
      `,
      params: [
        `${workspace.workspaceId}:${userId}`,
        workspace.workspaceId,
        userId,
        workspace.role,
        now,
        now,
      ],
    });
  }

  // Anything shared that the server no longer lists is access we have lost.
  statements.push({
    sql: `
      UPDATE workspace_memberships
      SET deleted_at = ?, updated_at = ?
      WHERE user_id = ?
        AND deleted_at IS NULL
        AND workspace_id IN (
          SELECT id FROM workspaces WHERE kind = 'shared'
        )
        ${keptIds.length > 0 ? `AND workspace_id NOT IN (${placeholders})` : ""}
    `,
    params: [now, now, userId, ...keptIds],
  });

  await executeTransaction(statements);
}
