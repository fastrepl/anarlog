import type { Session, SupabaseClient } from "@supabase/supabase-js";

export type TeamContext = {
  supabase: SupabaseClient;
  session: Session;
};

export type WorkspaceRole = "owner" | "admin" | "member";

export type WorkspaceMember = {
  userId: string;
  email: string;
  role: WorkspaceRole;
};

export type WorkspaceInvitation = {
  invitationId: string;
  email: string;
  expiresAt: string;
};

export type WorkspaceSeatUsage = {
  seatLimit: number | null;
  usedSeats: number;
  isBilled: boolean;
};

export class TeamError extends Error {
  constructor(message = "Workspace request failed") {
    super(message);
    this.name = "TeamError";
  }
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function requireTeamContext(auth: {
  supabase?: SupabaseClient | null;
  session?: Session | null;
}): TeamContext {
  if (!auth.supabase || !auth.session || auth.session.user.is_anonymous) {
    throw new TeamError();
  }
  return { supabase: auth.supabase, session: auth.session };
}

async function callRpc(
  context: TeamContext,
  functionName: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const response = await context.supabase
    .rpc(functionName, args)
    .setHeader("Authorization", `Bearer ${context.session.access_token}`);
  if (response.error !== null) {
    // Server messages carry the reason a manager needs (seat limit, permission)
    // and are written for humans, so they are surfaced rather than swallowed.
    throw new TeamError(response.error.message);
  }
  return response.data;
}

function rows(value: unknown): Record<string, unknown>[] {
  if (!Array.isArray(value)) throw new TeamError();
  return value as Record<string, unknown>[];
}

function text(value: unknown): string {
  if (typeof value !== "string" || value === "") throw new TeamError();
  return value;
}

function role(value: unknown): WorkspaceRole {
  if (value !== "owner" && value !== "admin" && value !== "member") {
    throw new TeamError();
  }
  return value;
}

export async function createWorkspace(context: TeamContext, name: string) {
  const row = rows(
    await callRpc(context, "create_workspace", { p_name: name }),
  )[0];
  if (!row) throw new TeamError();
  return { workspaceId: text(row.workspace_id) };
}

export async function renameWorkspace(
  context: TeamContext,
  workspaceId: string,
  name: string,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "rename_workspace", {
    p_workspace_id: workspaceId,
    p_name: name,
  });
}

export async function listWorkspaceMembers(
  context: TeamContext,
  workspaceId: string,
): Promise<WorkspaceMember[]> {
  assertWorkspaceId(workspaceId);
  return rows(
    await callRpc(context, "list_workspace_memberships", {
      p_workspace_id: workspaceId,
    }),
  )
    .filter((row) => row.deleted_at === null)
    .map((row) => ({
      userId: text(row.user_id),
      email: text(row.user_email),
      role: role(row.role),
    }));
}

export async function listWorkspaceInvitations(
  context: TeamContext,
  workspaceId: string,
): Promise<WorkspaceInvitation[]> {
  assertWorkspaceId(workspaceId);
  return rows(
    await callRpc(context, "list_workspace_invitations", {
      p_workspace_id: workspaceId,
    }),
  )
    .filter((row) => row.accepted_at === null && row.revoked_at === null)
    .map((row) => ({
      invitationId: text(row.invitation_id),
      email: text(row.invitee_email),
      expiresAt: text(row.expires_at),
    }));
}

export async function getSeatUsage(
  context: TeamContext,
  workspaceId: string,
): Promise<WorkspaceSeatUsage> {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "get_workspace_seat_usage", {
      p_workspace_id: workspaceId,
    }),
  )[0];
  if (!row) throw new TeamError();
  return {
    seatLimit: typeof row.seat_limit === "number" ? row.seat_limit : null,
    usedSeats: typeof row.used_seats === "number" ? row.used_seats : 0,
    isBilled: row.is_billed === true,
  };
}

export async function inviteMember(
  context: TeamContext,
  workspaceId: string,
  email: string,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "create_workspace_invitation", {
    p_workspace_id: workspaceId,
    p_invitee_email: email,
  });
}

export async function revokeInvitation(
  context: TeamContext,
  invitationId: string,
) {
  assertWorkspaceId(invitationId);
  await callRpc(context, "revoke_workspace_invitation", {
    p_invitation_id: invitationId,
  });
}

export async function removeMember(
  context: TeamContext,
  workspaceId: string,
  userId: string,
) {
  assertWorkspaceId(workspaceId);
  assertWorkspaceId(userId);
  await callRpc(context, "revoke_workspace_membership", {
    p_workspace_id: workspaceId,
    p_user_id: userId,
  });
}

export async function setMemberRole(
  context: TeamContext,
  workspaceId: string,
  userId: string,
  nextRole: "admin" | "member",
) {
  assertWorkspaceId(workspaceId);
  assertWorkspaceId(userId);
  await callRpc(context, "set_workspace_membership_role", {
    p_workspace_id: workspaceId,
    p_user_id: userId,
    p_role: nextRole,
  });
}

export async function transferOwnership(
  context: TeamContext,
  workspaceId: string,
  userId: string,
) {
  assertWorkspaceId(workspaceId);
  assertWorkspaceId(userId);
  await callRpc(context, "transfer_workspace_ownership", {
    p_workspace_id: workspaceId,
    p_user_id: userId,
  });
}

export async function leaveWorkspace(
  context: TeamContext,
  workspaceId: string,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "leave_workspace", { p_workspace_id: workspaceId });
}

export async function deleteWorkspace(
  context: TeamContext,
  workspaceId: string,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "delete_workspace", { p_workspace_id: workspaceId });
}

export async function listMyWorkspaces(context: TeamContext) {
  // RLS limits the embedded memberships to this account's own row, so the join
  // yields the caller's role without needing manager-only RPCs.
  const response = await context.supabase
    .from("workspaces")
    .select("id,name,kind,owner_user_id,workspace_memberships(role)")
    .eq("kind", "shared");
  if (response.error !== null) throw new TeamError(response.error.message);
  return (response.data ?? []).map((row) => {
    const memberships = Array.isArray(row.workspace_memberships)
      ? row.workspace_memberships
      : [];
    const ownerUserId = text(row.owner_user_id);
    return {
      workspaceId: text(row.id),
      name: text(row.name),
      ownerUserId,
      role: role(memberships[0]?.role ?? "member"),
      isOwner: ownerUserId === context.session.user.id,
    };
  });
}

function assertWorkspaceId(value: string) {
  if (!UUID_PATTERN.test(value)) throw new TeamError();
}
