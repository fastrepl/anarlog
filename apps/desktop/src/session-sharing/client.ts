import type { Session, SupabaseClient } from "@supabase/supabase-js";

import type { JSONContent } from "@hypr/editor/note";

import type { SharedNoteAttachment } from "~/shared-notes/cache";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PUBLIC_SLUG_PATTERN = /^s_[0-9a-f]{32}$/;
const CAPABILITY_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;
const MAX_ACCESS_ROWS = 1_000;
const MAX_RPC_DATA_BYTES = 1024 * 1024;
const MAX_COMMENT_BODY_BYTES = 16_384;
const MAX_COMMENT_ANCHOR_EXACT_BYTES = 4_096;
const MAX_COMMENT_ANCHOR_CONTEXT_BYTES = 256;
// callRpc rejects responses over MAX_RPC_DATA_BYTES; a comment row can
// approach 21 KiB (body + anchor quotes), so pages stay at 30 + 1 lookahead.
const COMMENT_PAGE_SIZE = 30;
const MAX_SNAPSHOT_BODY_BYTES = 2 * 1024 * 1024;
const MAX_SNAPSHOT_RESPONSE_BYTES = MAX_SNAPSHOT_BODY_BYTES + 256 * 1024;
const MAX_SNAPSHOT_TITLE_BYTES = 4_096;
const MAX_ACCESS_TOKEN_BYTES = 16 * 1024;
const SNAPSHOT_TIMEOUT_MS = 10_000;

const scopes = ["restricted", "workspace", "link", "public"] as const;
const settableScopes = ["restricted", "workspace", "public"] as const;
const capabilities = ["viewer", "commenter", "editor"] as const;

export type SessionShareScope = (typeof scopes)[number];
export type SettableSessionShareScope = (typeof settableScopes)[number];
export type SessionAccessCapability = (typeof capabilities)[number];
export type ShareManagementContext = {
  supabase: SupabaseClient;
  session: Session;
  signal?: AbortSignal;
};

export type CreatedSessionShare = {
  shareId: string;
  generalScope: SessionShareScope;
  publicSlug: string;
  accessVersion: number;
  wasCreated: boolean;
};

export type SessionShareManagement = {
  shareId: string;
  workspaceId: string;
  sessionId: string;
  generalScope: SessionShareScope;
  generalWorkspaceId: string | null;
  publicSlug: string;
  hasActiveLink: boolean;
  accessVersion: number;
};

export type SessionShareDeletionResult =
  | {
      shareId: null;
      accessVersion: null;
      deletedAt: null;
      wasDeleted: false;
    }
  | {
      shareId: string;
      accessVersion: number;
      deletedAt: string;
      wasDeleted: boolean;
    };

export type SessionShareScopeResult = {
  shareId: string;
  generalScope: SettableSessionShareScope;
  generalWorkspaceId: string | null;
  publicSlug: string;
  accessVersion: number;
};

export type SessionShareLinkResult = {
  shareId: string;
  linkId: string;
  linkToken: string | null;
  accessVersion: number;
  wasCreated: boolean;
};

export type SessionAccessInvitationResult = {
  invitationId: string;
  inviteToken: string | null;
  invitationExpiresAt: string;
  wasCreated: boolean;
};

export type SendSessionAccessInvitationEmailInput = {
  apiBaseUrl: string;
  session: Session;
  shareId: string;
  invitationId: string;
  inviteToken: string;
  noteTitle: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
};

export type SessionShareAccessEntry =
  | {
      entryType: "grant";
      entryId: string;
      userId: string;
      userEmail: string | null;
      capability: SessionAccessCapability;
      status: "active";
      createdAt: string;
      expiresAt: null;
    }
  | {
      entryType: "invitation";
      entryId: string;
      userId: string | null;
      userEmail: string;
      capability: SessionAccessCapability;
      status: "pending";
      createdAt: string;
      expiresAt: string;
    }
  | {
      entryType: "request";
      entryId: string;
      userId: string | null;
      userEmail: string | null;
      capability: SessionAccessCapability;
      status: "pending";
      createdAt: string;
      expiresAt: null;
    };

export type SessionShareCommentAnchor = {
  quoteExact: string;
  quotePrefix: string;
  quoteSuffix: string;
  fromHint: number | null;
  toHint: number | null;
};

export type SessionShareComment = {
  commentId: string;
  isAuthor: boolean;
  snapshotContentRevision: number;
  body: string;
  anchor: SessionShareCommentAnchor | null;
  createdAt: string;
};

export type SessionShareCommentPage = {
  comments: SessionShareComment[];
  nextCursor: { beforeCreatedAt: string; beforeCommentId: string } | null;
};

export type PublishedSessionShareSnapshot = {
  shareId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: JSONContent;
  attachments: SharedNoteAttachment[];
  webEditable: boolean;
  accessVersion: number;
  publishedAt: string;
};

export type PublishSessionShareSnapshotInput = {
  apiBaseUrl: string;
  session: Session;
  shareId: string;
  baseRevision: number;
  mutationId: string;
  title: string;
  body: unknown;
  attachmentIds?: string[];
  signal?: AbortSignal;
  fetcher?: typeof fetch;
};

export class ShareManagementError extends Error {
  constructor() {
    super("Share management is unavailable");
    this.name = "ShareManagementError";
  }
}

export class ShareSnapshotConflictError extends ShareManagementError {
  constructor(public readonly snapshot: PublishedSessionShareSnapshot) {
    super();
    this.name = "ShareSnapshotConflictError";
  }
}

export async function createOrReuseSessionShare(
  context: ShareManagementContext,
  input: { workspaceId: string; sessionId: string },
): Promise<CreatedSessionShare> {
  assertUuid(input.workspaceId);
  assertSessionId(input.sessionId);
  const data = await callRpc(context, "create_session_share", {
    p_workspace_id: input.workspaceId,
    p_session_id: input.sessionId,
  });
  return parseCreatedSessionShare(singleRow(data));
}

export async function getSessionShareManagement(
  context: ShareManagementContext,
  shareId: string,
): Promise<SessionShareManagement> {
  assertUuid(shareId);
  const data = await callRpc(context, "get_session_share_management", {
    p_share_id: shareId,
  });
  const result = parseSessionShareManagement(singleRow(data));
  if (result.shareId !== shareId) {
    throw unavailable();
  }
  return result;
}

export async function deleteSessionShareBySession(
  context: ShareManagementContext,
  input: { workspaceId: string; sessionId: string },
): Promise<SessionShareDeletionResult> {
  assertUuid(input.workspaceId);
  assertSessionId(input.sessionId);
  const data = await callRpc(context, "delete_session_share_by_session", {
    p_workspace_id: input.workspaceId,
    p_session_id: input.sessionId,
  });
  return parseSessionShareDeletionResult(singleRow(data));
}

export async function setSessionShareScope(
  context: ShareManagementContext,
  input: {
    shareId: string;
    scope: SettableSessionShareScope;
    workspaceId?: string | null;
  },
): Promise<SessionShareScopeResult> {
  assertUuid(input.shareId);
  assertOneOf(input.scope, settableScopes);
  const workspaceId = input.workspaceId ?? null;
  if (input.scope === "workspace") {
    assertUuid(workspaceId);
  } else if (workspaceId !== null) {
    throw unavailable();
  }

  const data = await callRpc(context, "set_session_share_scope", {
    p_share_id: input.shareId,
    p_general_scope: input.scope,
    p_general_workspace_id: workspaceId,
  });
  const result = parseSessionShareScopeResult(singleRow(data));
  if (
    result.shareId !== input.shareId ||
    result.generalScope !== input.scope ||
    result.generalWorkspaceId !== workspaceId
  ) {
    throw unavailable();
  }
  return result;
}

export async function enableSessionShareLink(
  context: ShareManagementContext,
  shareId: string,
): Promise<SessionShareLinkResult> {
  return issueSessionShareLink(context, shareId, "enable_session_share_link");
}

export async function rotateSessionShareLink(
  context: ShareManagementContext,
  shareId: string,
): Promise<SessionShareLinkResult & { linkToken: string; wasCreated: true }> {
  const result = await issueSessionShareLink(
    context,
    shareId,
    "rotate_session_share_link",
  );
  if (!result.wasCreated || result.linkToken === null) {
    throw unavailable();
  }
  return { ...result, linkToken: result.linkToken, wasCreated: true };
}

export async function createSessionAccessInvitation(
  context: ShareManagementContext,
  input: {
    shareId: string;
    inviteeEmail: string;
    capability: SessionAccessCapability;
  },
): Promise<SessionAccessInvitationResult> {
  assertUuid(input.shareId);
  const inviteeEmail = normalizeEmail(input.inviteeEmail);
  assertOneOf(input.capability, capabilities);
  const data = await callRpc(context, "create_session_access_invitation", {
    p_share_id: input.shareId,
    p_invitee_email: inviteeEmail,
    p_capability: input.capability,
  });
  return parseSessionAccessInvitationResult(singleRow(data));
}

export async function sendSessionAccessInvitationEmail(
  input: SendSessionAccessInvitationEmailInput,
): Promise<void> {
  try {
    assertAuthenticatedSession(input.session);
    assertUuid(input.shareId);
    assertUuid(input.invitationId);
    const inviteToken = expectCapabilityToken(input.inviteToken);
    const noteTitle = normalizeTitle(input.noteTitle);
    const body = JSON.stringify({
      shareId: input.shareId,
      inviteToken,
      noteTitle,
    });
    if (utf8Length(body) > 8 * 1024) throw unavailable();
    const request = createTimedSignal(input.signal);
    try {
      const response = await (input.fetcher ?? fetch)(
        invitationEmailUrl(input.apiBaseUrl, input.invitationId),
        {
          method: "POST",
          headers: {
            Accept: "application/json",
            Authorization: `Bearer ${input.session.access_token}`,
            "Content-Type": "application/json",
          },
          body,
          cache: "no-store",
          credentials: "omit",
          redirect: "error",
          referrerPolicy: "no-referrer",
          signal: request.signal,
        },
      );
      if (response.status !== 204) throw unavailable();
    } finally {
      request.dispose();
    }
  } catch (error) {
    if (error instanceof ShareManagementError) throw error;
    throw unavailable();
  }
}

export async function resendSessionAccessInvitation(
  context: ShareManagementContext,
  invitationId: string,
): Promise<{
  invitationId: string;
  inviteToken: string;
  invitationExpiresAt: string;
}> {
  assertUuid(invitationId);
  const data = await callRpc(context, "resend_session_access_invitation", {
    p_invitation_id: invitationId,
  });
  const row = expectRecord(singleRow(data), [
    "invitation_id",
    "invite_token",
    "invitation_expires_at",
  ]);
  const result = {
    invitationId: expectUuid(row.invitation_id),
    inviteToken: expectCapabilityToken(row.invite_token),
    invitationExpiresAt: expectTimestamp(row.invitation_expires_at),
  };
  if (result.invitationId !== invitationId) {
    throw unavailable();
  }
  return result;
}

export async function revokeSessionAccessInvitation(
  context: ShareManagementContext,
  invitationId: string,
): Promise<{ invitationId: string; revokedAt: string }> {
  assertUuid(invitationId);
  const data = await callRpc(context, "revoke_session_access_invitation", {
    p_invitation_id: invitationId,
  });
  const row = expectRecord(singleRow(data), ["invitation_id", "revoked_at"]);
  const result = {
    invitationId: expectUuid(row.invitation_id),
    revokedAt: expectTimestamp(row.revoked_at),
  };
  if (result.invitationId !== invitationId) {
    throw unavailable();
  }
  return result;
}

export async function listSessionShareAccess(
  context: ShareManagementContext,
  shareId: string,
): Promise<SessionShareAccessEntry[]> {
  assertUuid(shareId);
  const data = await callRpc(context, "list_session_share_access", {
    p_share_id: shareId,
  });
  if (!Array.isArray(data) || data.length > MAX_ACCESS_ROWS) {
    throw unavailable();
  }
  return data.map(parseSessionShareAccessEntry);
}

export async function createSessionShareComment(
  context: ShareManagementContext,
  input: {
    shareId: string;
    body: string;
    anchor?: SessionShareCommentAnchor | null;
  },
): Promise<SessionShareComment> {
  assertUuid(input.shareId);
  const body = normalizeCommentBody(input.body);
  const anchor = input.anchor ?? null;
  if (anchor !== null) {
    assertCommentAnchor(anchor);
  }
  const data = await callRpc(context, "create_session_share_comment", {
    p_share_id: input.shareId,
    p_body: body,
    p_anchor_quote_exact: anchor?.quoteExact ?? null,
    p_anchor_quote_prefix: anchor?.quotePrefix ?? null,
    p_anchor_quote_suffix: anchor?.quoteSuffix ?? null,
    p_anchor_from_hint: anchor?.fromHint ?? null,
    p_anchor_to_hint: anchor?.toHint ?? null,
  });
  const comment = parseSessionShareComment(singleRow(data));
  if (
    comment.isAuthor !== true ||
    (comment.anchor === null) !== (anchor === null)
  ) {
    throw unavailable();
  }
  return comment;
}

export async function listSessionShareComments(
  context: ShareManagementContext,
  input: {
    shareId: string;
    before?: { beforeCreatedAt: string; beforeCommentId: string } | null;
  },
): Promise<SessionShareCommentPage> {
  assertUuid(input.shareId);
  const before = input.before ?? null;
  if (before !== null) {
    expectTimestamp(before.beforeCreatedAt);
    assertUuid(before.beforeCommentId);
  }
  const data = await callRpc(context, "list_session_share_comments", {
    p_share_id: input.shareId,
    p_before_created_at: before?.beforeCreatedAt ?? null,
    p_before_comment_id: before?.beforeCommentId ?? null,
    p_limit: COMMENT_PAGE_SIZE + 1,
  });
  if (!Array.isArray(data) || data.length > COMMENT_PAGE_SIZE + 1) {
    throw unavailable();
  }
  const newestFirst = data.map(parseSessionShareComment);
  const kept = newestFirst.slice(0, COMMENT_PAGE_SIZE);
  const oldestKept = kept[kept.length - 1];
  return {
    comments: [...kept].reverse(),
    nextCursor:
      newestFirst.length > kept.length && oldestKept
        ? {
            beforeCreatedAt: oldestKept.createdAt,
            beforeCommentId: oldestKept.commentId,
          }
        : null,
  };
}

export async function deleteSessionShareComment(
  context: ShareManagementContext,
  commentId: string,
): Promise<{ commentId: string; deletedAt: string }> {
  assertUuid(commentId);
  const data = await callRpc(context, "delete_session_share_comment", {
    p_comment_id: commentId,
  });
  const row = expectRecord(singleRow(data), ["comment_id", "deleted_at"]);
  const result = {
    commentId: expectUuid(row.comment_id),
    deletedAt: expectTimestamp(row.deleted_at),
  };
  if (result.commentId !== commentId) {
    throw unavailable();
  }
  return result;
}

export async function updateSessionAccessGrant(
  context: ShareManagementContext,
  input: { grantId: string; capability: SessionAccessCapability },
): Promise<{
  grantId: string;
  capability: SessionAccessCapability;
  accessVersion: number;
}> {
  assertUuid(input.grantId);
  assertOneOf(input.capability, capabilities);
  const data = await callRpc(context, "update_session_access_grant", {
    p_grant_id: input.grantId,
    p_capability: input.capability,
  });
  const row = expectRecord(singleRow(data), [
    "grant_id",
    "capability",
    "access_version",
  ]);
  const result = {
    grantId: expectUuid(row.grant_id),
    capability: expectCapability(row.capability),
    accessVersion: expectPositiveInteger(row.access_version),
  };
  if (
    result.grantId !== input.grantId ||
    result.capability !== input.capability
  ) {
    throw unavailable();
  }
  return result;
}

export async function revokeSessionAccessGrant(
  context: ShareManagementContext,
  grantId: string,
): Promise<{ grantId: string; revokedAt: string; accessVersion: number }> {
  assertUuid(grantId);
  const data = await callRpc(context, "revoke_session_access_grant", {
    p_grant_id: grantId,
  });
  const row = expectRecord(singleRow(data), [
    "grant_id",
    "revoked_at",
    "access_version",
  ]);
  const result = {
    grantId: expectUuid(row.grant_id),
    revokedAt: expectTimestamp(row.revoked_at),
    accessVersion: expectPositiveInteger(row.access_version),
  };
  if (result.grantId !== grantId) {
    throw unavailable();
  }
  return result;
}

export async function reviewSessionAccessRequest(
  context: ShareManagementContext,
  input:
    | {
        requestId: string;
        decision: "approve";
        capability: SessionAccessCapability;
      }
    | { requestId: string; decision: "deny"; capability?: null },
): Promise<
  | {
      requestId: string;
      status: "approved";
      grantId: string;
      capability: SessionAccessCapability;
    }
  | {
      requestId: string;
      status: "denied";
      grantId: null;
      capability: null;
    }
> {
  assertUuid(input.requestId);
  const capability = input.decision === "approve" ? input.capability : null;
  if (capability !== null) {
    assertOneOf(capability, capabilities);
  }
  const data = await callRpc(context, "review_session_access_request", {
    p_request_id: input.requestId,
    p_decision: input.decision === "approve" ? "approved" : "denied",
    p_capability: capability,
  });
  const row = expectRecord(singleRow(data), [
    "request_id",
    "status",
    "grant_id",
    "capability",
  ]);
  const requestId = expectUuid(row.request_id);
  if (requestId !== input.requestId) {
    throw unavailable();
  }

  if (row.status === "approved") {
    const result = {
      requestId,
      status: "approved" as const,
      grantId: expectUuid(row.grant_id),
      capability: expectCapability(row.capability),
    };
    if (
      input.decision !== "approve" ||
      result.capability !== input.capability
    ) {
      throw unavailable();
    }
    return result;
  }
  if (
    row.status !== "denied" ||
    row.grant_id !== null ||
    row.capability !== null ||
    input.decision !== "deny"
  ) {
    throw unavailable();
  }
  return {
    requestId,
    status: "denied",
    grantId: null,
    capability: null,
  };
}

export async function publishSessionShareSnapshot(
  input: PublishSessionShareSnapshotInput,
): Promise<PublishedSessionShareSnapshot> {
  try {
    assertAuthenticatedSession(input.session);
    assertUuid(input.shareId);
    if (!Number.isSafeInteger(input.baseRevision) || input.baseRevision < 0) {
      throw unavailable();
    }
    assertUuid(input.mutationId);
    const title = normalizeTitle(input.title);
    const body = parseSessionShareDocument(input.body);
    const attachmentIds =
      input.attachmentIds === undefined
        ? undefined
        : normalizeAttachmentIds(input.attachmentIds);
    const url = snapshotUrl(input.apiBaseUrl, input.shareId);
    const requestBody = JSON.stringify({
      baseRevision: input.baseRevision,
      mutationId: input.mutationId,
      title,
      body,
      ...(attachmentIds === undefined ? {} : { attachmentIds }),
    });
    if (utf8Length(requestBody) > MAX_SNAPSHOT_RESPONSE_BYTES) {
      throw unavailable();
    }
    const request = createTimedSignal(input.signal);
    try {
      const response = await (input.fetcher ?? fetch)(url, {
        method: "PUT",
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${input.session.access_token}`,
          "Content-Type": "application/json",
        },
        body: requestBody,
        cache: "no-store",
        credentials: "omit",
        redirect: "error",
        referrerPolicy: "no-referrer",
        signal: request.signal,
      });
      if (!response.ok) {
        if (response.status === 409) {
          const contentType = response.headers.get("content-type");
          if (!contentType?.toLowerCase().includes("application/json")) {
            throw unavailable();
          }
          const responseText = await readLimitedResponse(
            response,
            MAX_SNAPSHOT_RESPONSE_BYTES,
          );
          const conflict = expectRecord(JSON.parse(responseText), [
            "code",
            "snapshot",
          ]);
          if (conflict.code !== "snapshot_conflict") throw unavailable();
          throw new ShareSnapshotConflictError(
            parsePublishedSessionShareSnapshot(conflict.snapshot),
          );
        }
        throw unavailable();
      }
      const contentType = response.headers.get("content-type");
      if (!contentType?.toLowerCase().includes("application/json")) {
        throw unavailable();
      }
      const responseText = await readLimitedResponse(
        response,
        MAX_SNAPSHOT_RESPONSE_BYTES,
      );
      const value: unknown = JSON.parse(responseText);
      const snapshot = parsePublishedSessionShareSnapshot(value);
      if (snapshot.shareId !== input.shareId) {
        throw unavailable();
      }
      return snapshot;
    } finally {
      request.dispose();
    }
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
}

export async function publishBeforeAccessMutation<T>({
  snapshot,
  mutateAccess,
}: {
  snapshot: PublishSessionShareSnapshotInput;
  mutateAccess: () => Promise<T>;
}): Promise<T> {
  await publishSessionShareSnapshot(snapshot);
  return mutateAccess();
}

export function parseSessionShareDocument(value: unknown): JSONContent {
  try {
    let parsed: unknown;
    if (typeof value === "string") {
      if (utf8Length(value) > MAX_SNAPSHOT_BODY_BYTES) {
        throw unavailable();
      }
      parsed = JSON.parse(value);
    } else {
      const encoded = JSON.stringify(value);
      if (utf8Length(encoded) > MAX_SNAPSHOT_BODY_BYTES) {
        throw unavailable();
      }
      parsed = JSON.parse(encoded);
    }
    if (
      !isRecord(parsed) ||
      parsed.type !== "doc" ||
      (parsed.content !== undefined && !Array.isArray(parsed.content))
    ) {
      throw unavailable();
    }
    return parsed as JSONContent;
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
}

async function issueSessionShareLink(
  context: ShareManagementContext,
  shareId: string,
  functionName: "enable_session_share_link" | "rotate_session_share_link",
): Promise<SessionShareLinkResult> {
  assertUuid(shareId);
  const data = await callRpc(context, functionName, { p_share_id: shareId });
  const result = parseSessionShareLinkResult(singleRow(data));
  if (result.shareId !== shareId) {
    throw unavailable();
  }
  return result;
}

async function callRpc(
  context: ShareManagementContext,
  functionName: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  try {
    assertAuthenticatedSession(context.session);
    const request = createTimedSignal(context.signal);
    try {
      const response: { data: unknown; error: unknown } = await context.supabase
        .rpc(functionName, args)
        .setHeader("Authorization", `Bearer ${context.session.access_token}`)
        .abortSignal(request.signal);
      if (response.error !== null) {
        throw unavailable();
      }
      assertJsonSize(response.data, MAX_RPC_DATA_BYTES);
      return response.data;
    } finally {
      request.dispose();
    }
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
}

function parseCreatedSessionShare(value: unknown): CreatedSessionShare {
  const row = expectRecord(value, [
    "share_id",
    "general_scope",
    "public_slug",
    "access_version",
    "was_created",
  ]);
  return {
    shareId: expectUuid(row.share_id),
    generalScope: expectScope(row.general_scope),
    publicSlug: expectPublicSlug(row.public_slug),
    accessVersion: expectPositiveInteger(row.access_version),
    wasCreated: expectBoolean(row.was_created),
  };
}

function parseSessionShareManagement(value: unknown): SessionShareManagement {
  const row = expectRecord(value, [
    "share_id",
    "workspace_id",
    "session_id",
    "general_scope",
    "general_workspace_id",
    "public_slug",
    "has_active_link",
    "access_version",
  ]);
  const generalScope = expectScope(row.general_scope);
  const generalWorkspaceId = expectNullableUuid(row.general_workspace_id);
  if ((generalScope === "workspace") !== (generalWorkspaceId !== null)) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.share_id),
    workspaceId: expectUuid(row.workspace_id),
    sessionId: expectSessionId(row.session_id),
    generalScope,
    generalWorkspaceId,
    publicSlug: expectPublicSlug(row.public_slug),
    hasActiveLink: expectBoolean(row.has_active_link),
    accessVersion: expectPositiveInteger(row.access_version),
  };
}

function parseSessionShareDeletionResult(
  value: unknown,
): SessionShareDeletionResult {
  const row = expectRecord(value, [
    "share_id",
    "access_version",
    "deleted_at",
    "was_deleted",
  ]);
  const wasDeleted = expectBoolean(row.was_deleted);
  if (row.share_id === null) {
    if (wasDeleted || row.access_version !== null || row.deleted_at !== null) {
      throw unavailable();
    }
    return {
      shareId: null,
      accessVersion: null,
      deletedAt: null,
      wasDeleted: false,
    };
  }
  return {
    shareId: expectUuid(row.share_id),
    accessVersion: expectPositiveInteger(row.access_version),
    deletedAt: expectTimestamp(row.deleted_at),
    wasDeleted,
  };
}

function parseSessionShareScopeResult(value: unknown): SessionShareScopeResult {
  const row = expectRecord(value, [
    "share_id",
    "general_scope",
    "general_workspace_id",
    "public_slug",
    "access_version",
  ]);
  const generalScope = expectOneOf(row.general_scope, settableScopes);
  const generalWorkspaceId = expectNullableUuid(row.general_workspace_id);
  if ((generalScope === "workspace") !== (generalWorkspaceId !== null)) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.share_id),
    generalScope,
    generalWorkspaceId,
    publicSlug: expectPublicSlug(row.public_slug),
    accessVersion: expectPositiveInteger(row.access_version),
  };
}

function parseSessionShareLinkResult(value: unknown): SessionShareLinkResult {
  const row = expectRecord(value, [
    "share_id",
    "link_id",
    "link_token",
    "access_version",
    "was_created",
  ]);
  const wasCreated = expectBoolean(row.was_created);
  const linkToken =
    row.link_token === null ? null : expectCapabilityToken(row.link_token);
  if (wasCreated !== (linkToken !== null)) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.share_id),
    linkId: expectUuid(row.link_id),
    linkToken,
    accessVersion: expectPositiveInteger(row.access_version),
    wasCreated,
  };
}

function parseSessionAccessInvitationResult(
  value: unknown,
): SessionAccessInvitationResult {
  const row = expectRecord(value, [
    "invitation_id",
    "invite_token",
    "invitation_expires_at",
    "was_created",
  ]);
  const wasCreated = expectBoolean(row.was_created);
  const inviteToken =
    row.invite_token === null ? null : expectCapabilityToken(row.invite_token);
  if (wasCreated !== (inviteToken !== null)) {
    throw unavailable();
  }
  return {
    invitationId: expectUuid(row.invitation_id),
    inviteToken,
    invitationExpiresAt: expectTimestamp(row.invitation_expires_at),
    wasCreated,
  };
}

function parseSessionShareAccessEntry(value: unknown): SessionShareAccessEntry {
  const row = expectRecord(value, [
    "entry_type",
    "entry_id",
    "user_id",
    "user_email",
    "capability",
    "status",
    "created_at",
    "expires_at",
  ]);
  const common = {
    entryId: expectUuid(row.entry_id),
    capability: expectCapability(row.capability),
    createdAt: expectTimestamp(row.created_at),
  };
  if (row.entry_type === "grant") {
    if (row.status !== "active" || row.expires_at !== null) {
      throw unavailable();
    }
    return {
      entryType: "grant",
      ...common,
      userId: expectUuid(row.user_id),
      userEmail: expectNullableEmail(row.user_email),
      status: "active",
      expiresAt: null,
    };
  }
  if (row.entry_type === "invitation") {
    if (row.status !== "pending") {
      throw unavailable();
    }
    return {
      entryType: "invitation",
      ...common,
      userId: expectNullableUuid(row.user_id),
      userEmail: expectEmail(row.user_email),
      status: "pending",
      expiresAt: expectTimestamp(row.expires_at),
    };
  }
  if (
    row.entry_type !== "request" ||
    row.status !== "pending" ||
    row.expires_at !== null
  ) {
    throw unavailable();
  }
  return {
    entryType: "request",
    ...common,
    userId: expectNullableUuid(row.user_id),
    userEmail: expectNullableEmail(row.user_email),
    status: "pending",
    expiresAt: null,
  };
}

export function parseSessionShareComment(value: unknown): SessionShareComment {
  const row = expectRecord(value, [
    "comment_id",
    "is_author",
    "snapshot_content_revision",
    "body",
    "anchor_quote_exact",
    "anchor_quote_prefix",
    "anchor_quote_suffix",
    "anchor_from_hint",
    "anchor_to_hint",
    "created_at",
  ]);
  return {
    commentId: expectUuid(row.comment_id),
    isAuthor: expectBoolean(row.is_author),
    snapshotContentRevision: expectPositiveInteger(
      row.snapshot_content_revision,
    ),
    body: expectCommentBody(row.body),
    anchor: parseCommentAnchorColumns(row),
    createdAt: expectTimestamp(row.created_at),
  };
}

function parseCommentAnchorColumns(
  row: Record<string, unknown>,
): SessionShareCommentAnchor | null {
  if (row.anchor_quote_exact === null) {
    if (
      row.anchor_quote_prefix !== null ||
      row.anchor_quote_suffix !== null ||
      row.anchor_from_hint !== null ||
      row.anchor_to_hint !== null
    ) {
      throw unavailable();
    }
    return null;
  }
  if (row.anchor_quote_prefix === null || row.anchor_quote_suffix === null) {
    throw unavailable();
  }
  return {
    quoteExact: expectAnchorQuote(row.anchor_quote_exact),
    quotePrefix: expectAnchorContext(row.anchor_quote_prefix),
    quoteSuffix: expectAnchorContext(row.anchor_quote_suffix),
    ...expectAnchorHints(row.anchor_from_hint, row.anchor_to_hint),
  };
}

function assertCommentAnchor(anchor: SessionShareCommentAnchor) {
  expectAnchorQuote(anchor.quoteExact);
  expectAnchorContext(anchor.quotePrefix);
  expectAnchorContext(anchor.quoteSuffix);
  expectAnchorHints(anchor.fromHint, anchor.toHint);
}

function expectAnchorQuote(value: unknown) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > MAX_COMMENT_ANCHOR_EXACT_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

function expectAnchorContext(value: unknown) {
  if (
    typeof value !== "string" ||
    utf8Length(value) > MAX_COMMENT_ANCHOR_CONTEXT_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

function expectAnchorHints(
  fromHint: unknown,
  toHint: unknown,
): { fromHint: number | null; toHint: number | null } {
  if (fromHint === null && toHint === null) {
    return { fromHint: null, toHint: null };
  }
  if (
    !Number.isSafeInteger(fromHint) ||
    !Number.isSafeInteger(toHint) ||
    (fromHint as number) < 1 ||
    (toHint as number) <= (fromHint as number)
  ) {
    throw unavailable();
  }
  return { fromHint: fromHint as number, toHint: toHint as number };
}

function normalizeCommentBody(value: string) {
  if (typeof value !== "string") {
    throw unavailable();
  }
  return expectCommentBody(value.trim());
}

function expectCommentBody(value: unknown) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > MAX_COMMENT_BODY_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

function parsePublishedSessionShareSnapshot(
  value: unknown,
): PublishedSessionShareSnapshot {
  const row = expectRecord(value, [
    "shareId",
    "schemaVersion",
    "contentRevision",
    "title",
    "body",
    "attachments",
    "webEditable",
    "accessVersion",
    "publishedAt",
  ]);
  if (row.schemaVersion !== 1) {
    throw unavailable();
  }
  return {
    shareId: expectUuid(row.shareId),
    schemaVersion: 1,
    contentRevision: expectPositiveInteger(row.contentRevision),
    title: expectTitle(row.title),
    body: parseSessionShareDocument(row.body),
    attachments: parseSharedNoteAttachments(row.attachments),
    webEditable: expectBoolean(row.webEditable),
    accessVersion: expectPositiveInteger(row.accessVersion),
    publishedAt: expectTimestamp(row.publishedAt),
  };
}

function normalizeAttachmentIds(value: string[]) {
  if (!Array.isArray(value) || value.length > 64) throw unavailable();
  const ids = value.map(expectUuid);
  if (new Set(ids).size !== ids.length) throw unavailable();
  return ids;
}

function parseSharedNoteAttachments(value: unknown): SharedNoteAttachment[] {
  if (!Array.isArray(value) || value.length > 64) throw unavailable();
  const ids = new Set<string>();
  return value.map((candidate) => {
    const row = expectRecord(candidate, [
      "id",
      "filename",
      "contentType",
      "sizeBytes",
      "sha256",
    ]);
    const id = expectUuid(row.id);
    if (ids.has(id)) throw unavailable();
    ids.add(id);
    if (
      typeof row.filename !== "string" ||
      row.filename.length === 0 ||
      row.filename.trim() !== row.filename ||
      utf8Length(row.filename) > 1024 ||
      /[\\/\u0000-\u001f\u007f]/.test(row.filename) ||
      typeof row.contentType !== "string" ||
      row.contentType.length === 0 ||
      row.contentType.length > 255 ||
      !Number.isSafeInteger(row.sizeBytes) ||
      (row.sizeBytes as number) < 1 ||
      (row.sizeBytes as number) > 512 * 1024 * 1024 ||
      typeof row.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/.test(row.sha256)
    ) {
      throw unavailable();
    }
    return {
      id,
      filename: row.filename,
      contentType: row.contentType,
      sizeBytes: row.sizeBytes as number,
      sha256: row.sha256,
    };
  });
}

function snapshotUrl(apiBaseUrl: string, shareId: string) {
  try {
    const base = new URL(apiBaseUrl);
    if (
      !["http:", "https:"].includes(base.protocol) ||
      base.username !== "" ||
      base.password !== "" ||
      base.search !== "" ||
      base.hash !== ""
    ) {
      throw unavailable();
    }
    return new URL(`/sync/shares/${shareId}/snapshot`, base.origin);
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
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
      throw unavailable();
    }
    return new URL(
      `/shared-notes/invitations/${invitationId}/email`,
      base.origin,
    );
  } catch (error) {
    if (error instanceof ShareManagementError) throw error;
    throw unavailable();
  }
}

function createTimedSignal(externalSignal?: AbortSignal) {
  const controller = new AbortController();
  const abort = () => controller.abort();
  if (externalSignal?.aborted) {
    controller.abort();
  } else {
    externalSignal?.addEventListener("abort", abort, { once: true });
  }
  const timeout = setTimeout(abort, SNAPSHOT_TIMEOUT_MS);
  return {
    signal: controller.signal,
    dispose: () => {
      clearTimeout(timeout);
      externalSignal?.removeEventListener("abort", abort);
    },
  };
}

async function readLimitedResponse(response: Response, limit: number) {
  const contentLength = response.headers.get("content-length");
  if (
    contentLength !== null &&
    (!/^\d+$/.test(contentLength) || Number(contentLength) > limit)
  ) {
    throw unavailable();
  }
  if (!response.body) {
    const text = await response.text();
    if (utf8Length(text) > limit) {
      throw unavailable();
    }
    return text;
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let bytes = 0;
  let text = "";
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    bytes += value.byteLength;
    if (bytes > limit) {
      await reader.cancel();
      throw unavailable();
    }
    text += decoder.decode(value, { stream: true });
  }
  return text + decoder.decode();
}

function singleRow(value: unknown) {
  if (!Array.isArray(value) || value.length !== 1) {
    throw unavailable();
  }
  return value[0];
}

function expectRecord(value: unknown, expectedKeys: readonly string[]) {
  if (!isRecord(value)) {
    throw unavailable();
  }
  const keys = Object.keys(value);
  if (
    keys.length !== expectedKeys.length ||
    expectedKeys.some(
      (key) => !Object.prototype.hasOwnProperty.call(value, key),
    )
  ) {
    throw unavailable();
  }
  return value;
}

function assertAuthenticatedSession(session: Session) {
  if (
    session.user.is_anonymous === true ||
    !UUID_PATTERN.test(session.user.id) ||
    typeof session.access_token !== "string" ||
    session.access_token === "" ||
    /[\u0000-\u001f\u007f]/.test(session.access_token) ||
    utf8Length(session.access_token) > MAX_ACCESS_TOKEN_BYTES
  ) {
    throw unavailable();
  }
}

function assertSessionId(value: unknown): asserts value is string {
  expectSessionId(value);
}

function expectSessionId(value: unknown) {
  if (
    typeof value !== "string" ||
    value === "" ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/.test(value) ||
    utf8Length(value) > 128
  ) {
    throw unavailable();
  }
  return value;
}

function assertUuid(value: unknown): asserts value is string {
  expectUuid(value);
}

function expectUuid(value: unknown) {
  if (typeof value !== "string" || !UUID_PATTERN.test(value)) {
    throw unavailable();
  }
  return value;
}

function expectNullableUuid(value: unknown) {
  return value === null ? null : expectUuid(value);
}

function expectPublicSlug(value: unknown) {
  if (typeof value !== "string" || !PUBLIC_SLUG_PATTERN.test(value)) {
    throw unavailable();
  }
  return value;
}

function expectScope(value: unknown) {
  return expectOneOf(value, scopes);
}

function expectCapability(value: unknown) {
  return expectOneOf(value, capabilities);
}

function assertOneOf<T extends string>(
  value: unknown,
  values: readonly T[],
): asserts value is T {
  expectOneOf(value, values);
}

function expectOneOf<T extends string>(value: unknown, values: readonly T[]) {
  if (typeof value !== "string" || !values.includes(value as T)) {
    throw unavailable();
  }
  return value as T;
}

function expectPositiveInteger(value: unknown) {
  if (!Number.isSafeInteger(value) || (value as number) < 1) {
    throw unavailable();
  }
  return value as number;
}

function expectBoolean(value: unknown) {
  if (typeof value !== "boolean") {
    throw unavailable();
  }
  return value;
}

function expectCapabilityToken(value: unknown) {
  if (typeof value !== "string" || !CAPABILITY_TOKEN_PATTERN.test(value)) {
    throw unavailable();
  }
  return value;
}

function expectTimestamp(value: unknown) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) {
    throw unavailable();
  }
  return value;
}

function normalizeEmail(value: string) {
  if (typeof value !== "string") {
    throw unavailable();
  }
  return expectEmail(value.trim().toLowerCase());
}

function expectEmail(value: unknown) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value.toLowerCase() !== value ||
    !/^[^\s@]+@[^\s@]+$/.test(value) ||
    /[\u0000-\u001f\u007f]/.test(value) ||
    utf8Length(value) > 320
  ) {
    throw unavailable();
  }
  return value;
}

function expectNullableEmail(value: unknown) {
  return value === null ? null : expectEmail(value);
}

function normalizeTitle(value: string) {
  if (typeof value !== "string") {
    throw unavailable();
  }
  return expectTitle(value.trim());
}

function expectTitle(value: unknown) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    utf8Length(value) > MAX_SNAPSHOT_TITLE_BYTES
  ) {
    throw unavailable();
  }
  return value;
}

function assertJsonSize(value: unknown, limit: number) {
  try {
    if (utf8Length(JSON.stringify(value)) > limit) {
      throw unavailable();
    }
  } catch (error) {
    if (error instanceof ShareManagementError) {
      throw error;
    }
    throw unavailable();
  }
}

function utf8Length(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function unavailable(): ShareManagementError {
  return new ShareManagementError();
}
