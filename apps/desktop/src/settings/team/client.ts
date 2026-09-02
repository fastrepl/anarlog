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

export const WORKSPACE_CAPABILITIES = [
  "team.shared_notes",
  "team.manage_workspace",
  "team.manage_members",
  "team.manage_policies",
  "team.view_usage",
  "team.custom_subdomain",
  "enterprise.sso",
  "enterprise.scim",
  "enterprise.retention",
  "enterprise.audit_logs",
  "enterprise.capture",
] as const;

export type WorkspaceCapability = (typeof WORKSPACE_CAPABILITIES)[number];
export type WorkspaceTier = "free" | "team" | "enterprise";

export type WorkspaceAccess = {
  role: WorkspaceRole;
  tier: WorkspaceTier;
  capabilities: WorkspaceCapability[];
  seatLimit: number | null;
  usedSeats: number;
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

export async function setWorkspaceLogo(
  context: TeamContext,
  workspaceId: string,
  logoDataUrl: string | null,
) {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "set_workspace_logo", {
      p_workspace_id: workspaceId,
      p_logo_data: logoDataUrl,
    }),
  )[0];
  if (!row) throw new TeamError();
  return {
    logoDataUrl:
      typeof row.workspace_logo_data === "string"
        ? row.workspace_logo_data
        : null,
  };
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

export async function getWorkspaceAccess(
  context: TeamContext,
  workspaceId: string,
): Promise<WorkspaceAccess> {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "get_workspace_access", {
      p_workspace_id: workspaceId,
    }),
  )[0];
  if (!row) throw new TeamError();
  const capabilities = Array.isArray(row.capabilities)
    ? row.capabilities.filter(isWorkspaceCapability)
    : [];
  return {
    role: role(row.workspace_role),
    tier: workspaceTier(row.workspace_tier),
    capabilities,
    seatLimit: typeof row.seat_limit === "number" ? row.seat_limit : null,
    usedSeats: typeof row.used_seats === "number" ? row.used_seats : 0,
  };
}

const INVITE_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;
const INVITATION_EMAIL_TIMEOUT_MS = 10_000;

export async function createWorkspaceInvitation(
  context: TeamContext,
  workspaceId: string,
  email: string,
) {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "create_workspace_invitation", {
      p_workspace_id: workspaceId,
      p_invitee_email: normalizeEmail(email),
    }),
  )[0];
  if (!row) throw new TeamError();
  const invitationId = text(row.invitation_id);
  assertWorkspaceId(invitationId);
  const inviteToken =
    row.invite_token === null || row.invite_token === undefined
      ? null
      : inviteTokenValue(row.invite_token);
  return {
    invitationId,
    inviteToken,
    expiresAt: text(row.invitation_expires_at),
    wasCreated: row.was_created === true,
  };
}

export async function resendWorkspaceInvitation(
  context: TeamContext,
  invitationId: string,
) {
  assertWorkspaceId(invitationId);
  const row = rows(
    await callRpc(context, "resend_workspace_invitation", {
      p_invitation_id: invitationId,
    }),
  )[0];
  if (!row) throw new TeamError();
  const nextInvitationId = text(row.invitation_id);
  assertWorkspaceId(nextInvitationId);
  return {
    invitationId: nextInvitationId,
    inviteToken: inviteTokenValue(row.invite_token),
    expiresAt: text(row.invitation_expires_at),
    wasCreated: true as const,
  };
}

export async function sendWorkspaceInvitationEmail(input: {
  apiBaseUrl: string;
  accessToken: string;
  workspaceId: string;
  invitationId: string;
  inviteToken: string;
  workspaceName: string;
  senderName: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
}) {
  assertWorkspaceId(input.workspaceId);
  assertWorkspaceId(input.invitationId);
  const inviteToken = inviteTokenValue(input.inviteToken);
  const body = JSON.stringify({
    workspaceId: input.workspaceId,
    inviteToken,
    workspaceName: input.workspaceName.trim(),
    fromName: input.senderName.trim(),
  });
  if (new TextEncoder().encode(body).byteLength > 8 * 1024) {
    throw new TeamError();
  }
  const url = invitationEmailUrl(input.apiBaseUrl, input.invitationId);
  const controller = new AbortController();
  const abort = () => controller.abort();
  if (input.signal?.aborted) controller.abort();
  else input.signal?.addEventListener("abort", abort, { once: true });
  const timeout = setTimeout(abort, INVITATION_EMAIL_TIMEOUT_MS);
  try {
    const response = await (input.fetcher ?? fetch)(url, {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${input.accessToken}`,
        "Content-Type": "application/json",
      },
      body,
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal: controller.signal,
    });
    if (response.status !== 204) throw new TeamError();
  } catch (error) {
    if (error instanceof TeamError) throw error;
    throw new TeamError();
  } finally {
    clearTimeout(timeout);
    input.signal?.removeEventListener("abort", abort);
  }
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

export type WorkspaceUsageOverview = {
  memberCount: number;
  pendingInvitations: number;
  enrolledDevices: number;
  sharesCreated30d: number;
  shareAccessEvents30d: number;
  seatLimit: number | null;
  usedSeats: number;
  isBilled: boolean;
};

export type WorkspacePolicy = {
  allowedShareScopes: Array<"restricted" | "workspace" | "link" | "public">;
  defaultShareScope: "restricted" | "workspace" | "link" | "public";
  retentionDays: number | null;
  modelTrainingOptOut: boolean;
  consentNotificationEnabled: boolean;
  requireSso: boolean;
};

function numberOrNull(value: unknown): number | null {
  return typeof value === "number" ? value : null;
}

function number(value: unknown): number {
  return typeof value === "number" ? value : 0;
}

export async function getWorkspaceUsageOverview(
  context: TeamContext,
  workspaceId: string,
): Promise<WorkspaceUsageOverview> {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "get_workspace_usage_overview", {
      p_workspace_id: workspaceId,
    }),
  )[0];
  if (!row) throw new TeamError();
  return {
    memberCount: number(row.member_count),
    pendingInvitations: number(row.pending_invitations),
    enrolledDevices: number(row.enrolled_devices),
    sharesCreated30d: number(row.shares_created_30d),
    shareAccessEvents30d: number(row.share_access_events_30d),
    seatLimit: numberOrNull(row.seat_limit),
    usedSeats: number(row.used_seats),
    isBilled: row.is_billed === true,
  };
}

function shareScope(
  value: unknown,
): "restricted" | "workspace" | "link" | "public" {
  if (
    value !== "restricted" &&
    value !== "workspace" &&
    value !== "link" &&
    value !== "public"
  ) {
    throw new TeamError();
  }
  return value;
}

export async function getWorkspacePolicy(
  context: TeamContext,
  workspaceId: string,
): Promise<WorkspacePolicy> {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "get_workspace_policy", {
      p_workspace_id: workspaceId,
    }),
  )[0];
  if (!row) throw new TeamError();
  const scopes = Array.isArray(row.allowed_share_scopes)
    ? row.allowed_share_scopes.map(shareScope)
    : (["restricted"] as WorkspacePolicy["allowedShareScopes"]);
  return {
    allowedShareScopes: scopes,
    defaultShareScope: shareScope(row.default_share_scope),
    retentionDays: numberOrNull(row.retention_days),
    modelTrainingOptOut: row.model_training_opt_out !== false,
    consentNotificationEnabled: row.consent_notification_enabled !== false,
    requireSso: row.require_sso === true,
  };
}

export function intersectAllowedShareScopes(
  policies: Array<Pick<WorkspacePolicy, "allowedShareScopes">>,
): WorkspacePolicy["allowedShareScopes"] {
  const scopes: WorkspacePolicy["allowedShareScopes"] = [
    "restricted",
    "workspace",
    "link",
    "public",
  ];
  if (policies.length === 0) return scopes;
  return scopes.filter((scope) =>
    policies.every((policy) => policy.allowedShareScopes.includes(scope)),
  );
}

export async function claimWorkspaceDomain(
  context: TeamContext,
  workspaceId: string,
  domain: string,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "claim_workspace_domain", {
    p_workspace_id: workspaceId,
    p_domain: domain,
  });
}

export async function setWorkspaceShareSlug(
  context: TeamContext,
  workspaceId: string,
  slug: string,
) {
  assertWorkspaceId(workspaceId);
  const row = rows(
    await callRpc(context, "set_workspace_share_slug", {
      p_workspace_id: workspaceId,
      p_slug: slug,
    }),
  )[0];
  if (!row) throw new TeamError();
  return {
    shareSlug: text(row.workspace_share_slug),
    shareBaseUrl: text(row.share_base_url),
  };
}

export async function rotateWorkspaceScimToken(
  context: TeamContext,
  workspaceId: string,
  domain: string,
  token: string,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "rotate_workspace_scim_token", {
    p_workspace_id: workspaceId,
    p_domain: domain,
    p_token: token,
  });
}

export async function setWorkspacePolicy(
  context: TeamContext,
  workspaceId: string,
  policy: WorkspacePolicy,
) {
  assertWorkspaceId(workspaceId);
  await callRpc(context, "set_workspace_policy", {
    p_workspace_id: workspaceId,
    p_allowed_share_scopes: policy.allowedShareScopes,
    p_default_share_scope: policy.defaultShareScope,
    p_retention_days: policy.retentionDays,
    p_model_training_opt_out: policy.modelTrainingOptOut,
    p_consent_notification_enabled: policy.consentNotificationEnabled,
    p_require_sso: policy.requireSso,
  });
}

export async function listMyWorkspaces(context: TeamContext) {
  // RLS limits the embedded memberships to this account's own row, so the join
  // yields the caller's role without needing manager-only RPCs.
  const response = await context.supabase
    .from("workspaces")
    .select(
      "id,name,kind,owner_user_id,share_slug,logo_data,workspace_memberships(role)",
    )
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
      shareSlug: typeof row.share_slug === "string" ? row.share_slug : null,
      logoDataUrl: typeof row.logo_data === "string" ? row.logo_data : null,
      role: role(memberships[0]?.role ?? "member"),
      isOwner: ownerUserId === context.session.user.id,
    };
  });
}

function normalizeEmail(value: string) {
  const email = value.trim().toLowerCase();
  if (
    email.length === 0 ||
    email.length > 320 ||
    !/^[^\s@]+@[^\s@]+$/.test(email) ||
    /[\u0000-\u001f\u007f]/.test(email)
  ) {
    throw new TeamError();
  }
  return email;
}

function isWorkspaceCapability(value: unknown): value is WorkspaceCapability {
  return WORKSPACE_CAPABILITIES.some((capability) => capability === value);
}

function workspaceTier(value: unknown): WorkspaceTier {
  if (value !== "free" && value !== "team" && value !== "enterprise") {
    throw new TeamError();
  }
  return value;
}

function inviteTokenValue(value: unknown) {
  if (typeof value !== "string" || !INVITE_TOKEN_PATTERN.test(value)) {
    throw new TeamError();
  }
  return value;
}

function invitationEmailUrl(apiBaseUrl: string, invitationId: string) {
  try {
    const base = new URL(apiBaseUrl);
    if (
      !["http:", "https:"].includes(base.protocol) ||
      base.username !== "" ||
      base.password !== "" ||
      base.search !== "" ||
      base.hash !== ""
    ) {
      throw new TeamError();
    }
    return new URL(
      `/workspaces/invitations/${invitationId}/email`,
      base.origin,
    );
  } catch (error) {
    if (error instanceof TeamError) throw error;
    throw new TeamError();
  }
}

function assertWorkspaceId(value: string) {
  if (!UUID_PATTERN.test(value)) throw new TeamError();
}
