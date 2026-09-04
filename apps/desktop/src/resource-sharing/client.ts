import type { Session, SupabaseClient } from "@supabase/supabase-js";

export const SHARED_RESOURCE_TYPES = [
  "folder",
  "template",
  "automation",
] as const;

export type SharedResourceType = (typeof SHARED_RESOURCE_TYPES)[number];

export type SharedResource = {
  shareId: string;
  resourceType: SharedResourceType;
  sourceId: string;
  title: string;
  payload: Record<string, unknown>;
  ownerUserId: string;
  ownerEmail: string;
  generalWorkspaceId: string | null;
  workspaceName: string | null;
  accessKind: "owner" | "team" | "guest";
  updatedAt: string;
};

export type SharedResourceGuest = {
  guestId: string;
  email: string;
  createdAt: string;
};

export type ResourceSharingContext = {
  supabase: SupabaseClient;
  session: Session;
};

export class ResourceSharingError extends Error {
  constructor(message = "Shared resource request failed") {
    super(message);
    this.name = "ResourceSharingError";
  }
}

export function requireResourceSharingContext(auth: {
  supabase?: SupabaseClient | null;
  session?: Session | null;
}): ResourceSharingContext {
  if (!auth.supabase || !auth.session || auth.session.user.is_anonymous) {
    throw new ResourceSharingError();
  }
  return { supabase: auth.supabase, session: auth.session };
}

export async function listSharedResources(
  context: ResourceSharingContext,
  resourceType: SharedResourceType,
): Promise<SharedResource[]> {
  return rows(
    await callRpc(context, "list_shared_resources", {
      p_resource_type: resourceType,
    }),
  ).map(parseSharedResource);
}

export async function upsertSharedResource(
  context: ResourceSharingContext,
  input: {
    resourceType: SharedResourceType;
    sourceId: string;
    title: string;
    payload: Record<string, unknown>;
    generalWorkspaceId: string | null;
  },
): Promise<{ shareId: string; generalWorkspaceId: string | null }> {
  const row = rows(
    await callRpc(context, "upsert_shared_resource", {
      p_resource_type: input.resourceType,
      p_source_id: input.sourceId,
      p_title: input.title,
      p_payload: input.payload,
      p_general_workspace_id: input.generalWorkspaceId,
    }),
  )[0];
  if (!row) throw new ResourceSharingError();
  return {
    shareId: text(row.share_id),
    generalWorkspaceId: nullableText(row.general_workspace_id),
  };
}

export async function moveSharedResource(
  context: ResourceSharingContext,
  input: {
    shareId: string;
    sourceId: string;
    title: string;
    payload: Record<string, unknown>;
  },
): Promise<void> {
  await callRpc(context, "move_shared_resource", {
    p_share_id: input.shareId,
    p_source_id: input.sourceId,
    p_title: input.title,
    p_payload: input.payload,
  });
}

export async function listSharedResourceGuests(
  context: ResourceSharingContext,
  shareId: string,
): Promise<SharedResourceGuest[]> {
  return rows(
    await callRpc(context, "list_shared_resource_guests", {
      p_share_id: shareId,
    }),
  ).map((row) => ({
    guestId: text(row.guest_id),
    email: text(row.invitee_email),
    createdAt: text(row.created_at),
  }));
}

export async function grantSharedResourceAccess(
  context: ResourceSharingContext,
  shareId: string,
  email: string,
): Promise<SharedResourceGuest> {
  const row = rows(
    await callRpc(context, "grant_shared_resource_access", {
      p_share_id: shareId,
      p_invitee_email: email.trim().toLowerCase(),
    }),
  )[0];
  if (!row) throw new ResourceSharingError();
  return {
    guestId: text(row.guest_id),
    email: text(row.invitee_email),
    createdAt: new Date().toISOString(),
  };
}

export async function revokeSharedResourceAccess(
  context: ResourceSharingContext,
  guestId: string,
): Promise<void> {
  await callRpc(context, "revoke_shared_resource_access", {
    p_guest_id: guestId,
  });
}

export async function deleteSharedResource(
  context: ResourceSharingContext,
  shareId: string,
): Promise<void> {
  await callRpc(context, "delete_shared_resource", { p_share_id: shareId });
}

export function isTeamSharingUpsellError(error: unknown): boolean {
  return (
    error instanceof Error &&
    (error.message.includes("multi-resource guest requires Team membership") ||
      error.message.includes("workspace capability required"))
  );
}

async function callRpc(
  context: ResourceSharingContext,
  functionName: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const response = await context.supabase
    .rpc(functionName, args)
    .setHeader("Authorization", `Bearer ${context.session.access_token}`);
  if (response.error !== null) {
    throw new ResourceSharingError(response.error.message);
  }
  return response.data;
}

function parseSharedResource(row: Record<string, unknown>): SharedResource {
  const resourceType = row.resource_type;
  const accessKind = row.access_kind;
  const payload = row.payload;
  if (
    !SHARED_RESOURCE_TYPES.includes(resourceType as SharedResourceType) ||
    (accessKind !== "owner" &&
      accessKind !== "team" &&
      accessKind !== "guest") ||
    !payload ||
    typeof payload !== "object" ||
    Array.isArray(payload)
  ) {
    throw new ResourceSharingError();
  }
  return {
    shareId: text(row.share_id),
    resourceType: resourceType as SharedResourceType,
    sourceId: text(row.source_id),
    title: text(row.resource_title),
    payload: payload as Record<string, unknown>,
    ownerUserId: text(row.owner_user_id),
    ownerEmail: text(row.owner_email),
    generalWorkspaceId: nullableText(row.general_workspace_id),
    workspaceName: nullableText(row.workspace_name),
    accessKind,
    updatedAt: text(row.updated_at),
  };
}

function rows(value: unknown): Record<string, unknown>[] {
  if (!Array.isArray(value)) throw new ResourceSharingError();
  return value as Record<string, unknown>[];
}

function text(value: unknown): string {
  if (typeof value !== "string" || value === "") {
    throw new ResourceSharingError();
  }
  return value;
}

function nullableText(value: unknown): string | null {
  if (value === null) return null;
  return text(value);
}
