import {
  COMMENT_PAGE_SIZE,
  MAX_ACCESS_ROWS,
  MAX_RPC_DATA_BYTES,
  MAX_SNAPSHOT_RESPONSE_BYTES,
  SNAPSHOT_TIMEOUT_MS,
  ShareManagementError,
  assertAuthenticatedSession,
  assertCommentAnchor,
  assertJsonSize,
  assertOneOf,
  assertSessionId,
  assertUuid,
  capabilities,
  expectCapability,
  expectCapabilityToken,
  expectPositiveInteger,
  expectRecord,
  expectTimestamp,
  expectUuid,
  normalizeAttachmentIds,
  normalizeCommentBody,
  normalizeEmail,
  normalizeTitle,
  parseCreatedSessionShare,
  parsePublishedSessionShareSnapshot,
  parseSessionAccessInvitationResult,
  parseSessionShareAccessEntry,
  parseSessionShareComment,
  parseSessionShareDeletionResult,
  parseSessionShareDocument,
  parseSessionShareLinkResult,
  parseSessionShareManagement,
  parseSessionShareScopeResult,
  settableScopes,
  singleRow,
  unavailable,
  utf8Length,
} from "./client-contract";
import type {
  CreatedSessionShare,
  PublishedSessionShareSnapshot,
  PublishSessionShareSnapshotInput,
  SendSessionAccessInvitationEmailInput,
  SessionAccessCapability,
  SessionAccessInvitationResult,
  SessionShareAccessEntry,
  SessionShareComment,
  SessionShareCommentAnchor,
  SessionShareCommentPage,
  SessionShareDeletionResult,
  SessionShareLinkResult,
  SessionShareManagement,
  SessionShareScopeResult,
  SettableSessionShareScope,
  ShareManagementContext,
} from "./client-contract";

export {
  ShareManagementError,
  parseSessionShareComment,
  parseSessionShareDocument,
} from "./client-contract";
export type {
  CreatedSessionShare,
  PublishedSessionShareSnapshot,
  PublishSessionShareSnapshotInput,
  SendSessionAccessInvitationEmailInput,
  SessionAccessCapability,
  SessionAccessInvitationResult,
  SessionShareAccessEntry,
  SessionShareComment,
  SessionShareCommentAnchor,
  SessionShareCommentPage,
  SessionShareDeletionResult,
  SessionShareLinkResult,
  SessionShareManagement,
  SessionShareScope,
  SessionShareScopeResult,
  SettableSessionShareScope,
  ShareManagementContext,
} from "./client-contract";

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
