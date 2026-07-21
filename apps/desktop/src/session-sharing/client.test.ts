import type { Session, SupabaseClient } from "@supabase/supabase-js";
import { describe, expect, it, vi } from "vitest";

import {
  createOrReuseSessionShare,
  createSessionAccessInvitation,
  createSessionShareComment,
  deleteSessionShareBySession,
  deleteSessionShareComment,
  enableSessionShareLink,
  getSessionShareManagement,
  listSessionShareAccess,
  listSessionShareComments,
  parseSessionShareDocument,
  publishBeforeAccessMutation,
  publishSessionShareSnapshot,
  resendSessionAccessInvitation,
  reviewSessionAccessRequest,
  revokeSessionAccessGrant,
  revokeSessionAccessInvitation,
  rotateSessionShareLink,
  setSessionShareScope,
  ShareManagementError,
  updateSessionAccessGrant,
} from "./client";

const userId = "11111111-1111-4111-8111-111111111111";
const workspaceId = "22222222-2222-4222-8222-222222222222";
const shareId = "33333333-3333-4333-8333-333333333333";
const linkId = "44444444-4444-4444-8444-444444444444";
const invitationId = "55555555-5555-4555-8555-555555555555";
const grantId = "66666666-6666-4666-8666-666666666666";
const requestId = "77777777-7777-4777-8777-777777777777";
const mutationId = "88888888-8888-4888-8888-888888888888";
const linkToken = "l".repeat(43);
const inviteToken = "i".repeat(43);
const publicSlug = `s_${"a".repeat(32)}`;
const timestamp = "2026-07-17T01:02:03.000Z";

function session(): Session {
  return {
    access_token: "authenticated-access-token",
    token_type: "bearer",
    user: { id: userId, is_anonymous: false },
  } as Session;
}

function rpcHarness(data: unknown, error: unknown = null) {
  const abortSignal = vi.fn().mockResolvedValue({ data, error });
  const setHeader = vi.fn(() => ({ abortSignal }));
  const rpc = vi.fn(() => ({ setHeader }));
  return {
    abortSignal,
    context: {
      supabase: { rpc } as unknown as SupabaseClient,
      session: session(),
    },
    rpc,
    setHeader,
  };
}

describe("session share management client", () => {
  it("creates or reuses a share through an explicitly authenticated RPC", async () => {
    const harness = rpcHarness([
      {
        share_id: shareId,
        general_scope: "public",
        public_slug: publicSlug,
        access_version: 4,
        was_created: false,
      },
    ]);

    await expect(
      createOrReuseSessionShare(harness.context, {
        workspaceId,
        sessionId: "local-session-id",
      }),
    ).resolves.toEqual({
      shareId,
      generalScope: "public",
      publicSlug,
      accessVersion: 4,
      wasCreated: false,
    });
    expect(harness.rpc).toHaveBeenCalledWith("create_session_share", {
      p_workspace_id: workspaceId,
      p_session_id: "local-session-id",
    });
    expect(harness.setHeader).toHaveBeenCalledWith(
      "Authorization",
      "Bearer authenticated-access-token",
    );
    expect(harness.abortSignal).toHaveBeenCalledWith(expect.any(AbortSignal));
  });

  it("strictly parses management and scope rows", async () => {
    const managementHarness = rpcHarness([
      {
        share_id: shareId,
        workspace_id: workspaceId,
        session_id: "local-session-id",
        general_scope: "restricted",
        general_workspace_id: null,
        public_slug: publicSlug,
        has_active_link: false,
        access_version: 1,
      },
    ]);
    await expect(
      getSessionShareManagement(managementHarness.context, shareId),
    ).resolves.toMatchObject({
      shareId,
      workspaceId,
      generalScope: "restricted",
      generalWorkspaceId: null,
    });

    const scopeHarness = rpcHarness([
      {
        share_id: shareId,
        general_scope: "workspace",
        general_workspace_id: workspaceId,
        public_slug: publicSlug,
        access_version: 2,
      },
    ]);
    await expect(
      setSessionShareScope(scopeHarness.context, {
        shareId,
        scope: "workspace",
        workspaceId,
      }),
    ).resolves.toMatchObject({
      generalScope: "workspace",
      generalWorkspaceId: workspaceId,
    });
  });

  it("deletes any owner share by its local source identity", async () => {
    const deletedHarness = rpcHarness([
      {
        share_id: shareId,
        access_version: 5,
        deleted_at: timestamp,
        was_deleted: true,
      },
    ]);

    await expect(
      deleteSessionShareBySession(deletedHarness.context, {
        workspaceId,
        sessionId: "local-session-id",
      }),
    ).resolves.toEqual({
      shareId,
      accessVersion: 5,
      deletedAt: timestamp,
      wasDeleted: true,
    });
    expect(deletedHarness.rpc).toHaveBeenCalledWith(
      "delete_session_share_by_session",
      {
        p_workspace_id: workspaceId,
        p_session_id: "local-session-id",
      },
    );

    const unsharedHarness = rpcHarness([
      {
        share_id: null,
        access_version: null,
        deleted_at: null,
        was_deleted: false,
      },
    ]);
    await expect(
      deleteSessionShareBySession(unsharedHarness.context, {
        workspaceId,
        sessionId: "local-session-id",
      }),
    ).resolves.toEqual({
      shareId: null,
      accessVersion: null,
      deletedAt: null,
      wasDeleted: false,
    });
  });

  it("keeps bearer link tokens ephemeral and validates enable versus rotate", async () => {
    const existingHarness = rpcHarness([
      {
        share_id: shareId,
        link_id: linkId,
        link_token: null,
        access_version: 3,
        was_created: false,
      },
    ]);
    await expect(
      enableSessionShareLink(existingHarness.context, shareId),
    ).resolves.toMatchObject({ linkToken: null, wasCreated: false });

    const rotateHarness = rpcHarness([
      {
        share_id: shareId,
        link_id: linkId,
        link_token: linkToken,
        access_version: 4,
        was_created: true,
      },
    ]);
    await expect(
      rotateSessionShareLink(rotateHarness.context, shareId),
    ).resolves.toMatchObject({ linkToken, wasCreated: true });
  });

  it("manages invitations, grants, and pending requests", async () => {
    const invitationHarness = rpcHarness([
      {
        invitation_id: invitationId,
        invite_token: inviteToken,
        invitation_expires_at: timestamp,
        was_created: true,
      },
    ]);
    await expect(
      createSessionAccessInvitation(invitationHarness.context, {
        shareId,
        inviteeEmail: " Person@Example.com ",
        capability: "commenter",
      }),
    ).resolves.toMatchObject({ invitationId, inviteToken, wasCreated: true });
    expect(invitationHarness.rpc).toHaveBeenCalledWith(
      "create_session_access_invitation",
      {
        p_share_id: shareId,
        p_invitee_email: "person@example.com",
        p_capability: "commenter",
      },
    );

    const resendHarness = rpcHarness([
      {
        invitation_id: invitationId,
        invite_token: inviteToken,
        invitation_expires_at: timestamp,
      },
    ]);
    await expect(
      resendSessionAccessInvitation(resendHarness.context, invitationId),
    ).resolves.toMatchObject({ invitationId, inviteToken });

    const revokeInviteHarness = rpcHarness([
      { invitation_id: invitationId, revoked_at: timestamp },
    ]);
    await expect(
      revokeSessionAccessInvitation(revokeInviteHarness.context, invitationId),
    ).resolves.toEqual({ invitationId, revokedAt: timestamp });

    const listHarness = rpcHarness([
      {
        entry_type: "grant",
        entry_id: grantId,
        user_id: userId,
        user_email: "person@example.com",
        capability: "viewer",
        status: "active",
        created_at: timestamp,
        expires_at: null,
      },
      {
        entry_type: "invitation",
        entry_id: invitationId,
        user_id: null,
        user_email: "invitee@example.com",
        capability: "commenter",
        status: "pending",
        created_at: timestamp,
        expires_at: timestamp,
      },
      {
        entry_type: "request",
        entry_id: requestId,
        user_id: userId,
        user_email: "requester@example.com",
        capability: "editor",
        status: "pending",
        created_at: timestamp,
        expires_at: null,
      },
    ]);
    await expect(
      listSessionShareAccess(listHarness.context, shareId),
    ).resolves.toEqual([
      expect.objectContaining({ entryType: "grant", entryId: grantId }),
      expect.objectContaining({
        entryType: "invitation",
        entryId: invitationId,
      }),
      expect.objectContaining({ entryType: "request", entryId: requestId }),
    ]);

    const updateHarness = rpcHarness([
      { grant_id: grantId, capability: "editor", access_version: 7 },
    ]);
    await expect(
      updateSessionAccessGrant(updateHarness.context, {
        grantId,
        capability: "editor",
      }),
    ).resolves.toEqual({ grantId, capability: "editor", accessVersion: 7 });

    const revokeGrantHarness = rpcHarness([
      { grant_id: grantId, revoked_at: timestamp, access_version: 8 },
    ]);
    await expect(
      revokeSessionAccessGrant(revokeGrantHarness.context, grantId),
    ).resolves.toEqual({ grantId, revokedAt: timestamp, accessVersion: 8 });

    const reviewHarness = rpcHarness([
      {
        request_id: requestId,
        status: "approved",
        grant_id: grantId,
        capability: "commenter",
      },
    ]);
    await expect(
      reviewSessionAccessRequest(reviewHarness.context, {
        requestId,
        decision: "approve",
        capability: "commenter",
      }),
    ).resolves.toEqual({
      requestId,
      status: "approved",
      grantId,
      capability: "commenter",
    });
    expect(reviewHarness.rpc).toHaveBeenCalledWith(
      "review_session_access_request",
      {
        p_request_id: requestId,
        p_decision: "approved",
        p_capability: "commenter",
      },
    );
  });

  it("maps RPC and parsing failures to a generic error without server details", async () => {
    const tokenBearingServerError = `do not expose ${linkToken}`;
    const failedHarness = rpcHarness(null, {
      message: tokenBearingServerError,
    });
    const error = await createOrReuseSessionShare(failedHarness.context, {
      workspaceId,
      sessionId: "local-session-id",
    }).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(ShareManagementError);
    expect((error as Error).message).toBe("Share management is unavailable");
    expect((error as Error).message).not.toContain(linkToken);

    const malformedHarness = rpcHarness([
      {
        share_id: shareId,
        general_scope: "restricted",
        public_slug: publicSlug,
        access_version: 1,
        was_created: true,
        unexpected: "field",
      },
    ]);
    await expect(
      createOrReuseSessionShare(malformedHarness.context, {
        workspaceId,
        sessionId: "local-session-id",
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);
  });
});

const commentId = "99999999-9999-4999-8999-999999999999";

function commentRow(overrides: Record<string, unknown> = {}) {
  return {
    comment_id: commentId,
    is_author: true,
    snapshot_content_revision: 3,
    body: "A comment",
    anchor_quote_exact: null,
    anchor_quote_prefix: null,
    anchor_quote_suffix: null,
    anchor_from_hint: null,
    anchor_to_hint: null,
    created_at: timestamp,
    ...overrides,
  };
}

function anchoredCommentRow(overrides: Record<string, unknown> = {}) {
  return commentRow({
    anchor_quote_exact: "quoted text",
    anchor_quote_prefix: "before ",
    anchor_quote_suffix: " after",
    anchor_from_hint: 4,
    anchor_to_hint: 15,
    ...overrides,
  });
}

describe("session share comments", () => {
  it("creates an anchored comment with all anchor parameters and an auth header", async () => {
    const harness = rpcHarness([anchoredCommentRow()]);

    await expect(
      createSessionShareComment(harness.context, {
        shareId,
        body: "  A comment  ",
        anchor: {
          quoteExact: "quoted text",
          quotePrefix: "before ",
          quoteSuffix: " after",
          fromHint: 4,
          toHint: 15,
        },
      }),
    ).resolves.toEqual({
      commentId,
      isAuthor: true,
      snapshotContentRevision: 3,
      body: "A comment",
      anchor: {
        quoteExact: "quoted text",
        quotePrefix: "before ",
        quoteSuffix: " after",
        fromHint: 4,
        toHint: 15,
      },
      createdAt: timestamp,
    });
    expect(harness.rpc).toHaveBeenCalledWith("create_session_share_comment", {
      p_share_id: shareId,
      p_body: "A comment",
      p_anchor_quote_exact: "quoted text",
      p_anchor_quote_prefix: "before ",
      p_anchor_quote_suffix: " after",
      p_anchor_from_hint: 4,
      p_anchor_to_hint: 15,
    });
    expect(harness.setHeader).toHaveBeenCalledWith(
      "Authorization",
      "Bearer authenticated-access-token",
    );
  });

  it("creates an unanchored comment with explicit null anchor parameters", async () => {
    const harness = rpcHarness([commentRow()]);

    await expect(
      createSessionShareComment(harness.context, {
        shareId,
        body: "A comment",
      }),
    ).resolves.toMatchObject({ commentId, anchor: null });
    expect(harness.rpc).toHaveBeenCalledWith("create_session_share_comment", {
      p_share_id: shareId,
      p_body: "A comment",
      p_anchor_quote_exact: null,
      p_anchor_quote_prefix: null,
      p_anchor_quote_suffix: null,
      p_anchor_from_hint: null,
      p_anchor_to_hint: null,
    });
  });

  it("rejects malformed comment rows and echo mismatches", async () => {
    const partialTrio = rpcHarness([
      anchoredCommentRow({ anchor_quote_suffix: null }),
    ]);
    await expect(
      createSessionShareComment(partialTrio.context, {
        shareId,
        body: "A comment",
        anchor: {
          quoteExact: "quoted text",
          quotePrefix: "before ",
          quoteSuffix: " after",
          fromHint: null,
          toHint: null,
        },
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);

    const hintsWithoutQuotes = rpcHarness([
      commentRow({ anchor_from_hint: 4, anchor_to_hint: 15 }),
    ]);
    await expect(
      createSessionShareComment(hintsWithoutQuotes.context, {
        shareId,
        body: "A comment",
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);

    const extraKey = rpcHarness([commentRow({ unexpected: "field" })]);
    await expect(
      createSessionShareComment(extraKey.context, {
        shareId,
        body: "A comment",
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);

    const { created_at: _dropped, ...missingKeyRow } = commentRow();
    const missingKey = rpcHarness([missingKeyRow]);
    await expect(
      createSessionShareComment(missingKey.context, {
        shareId,
        body: "A comment",
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);

    const notAuthor = rpcHarness([commentRow({ is_author: false })]);
    await expect(
      createSessionShareComment(notAuthor.context, {
        shareId,
        body: "A comment",
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);

    const anchorDropped = rpcHarness([commentRow()]);
    await expect(
      createSessionShareComment(anchorDropped.context, {
        shareId,
        body: "A comment",
        anchor: {
          quoteExact: "quoted text",
          quotePrefix: "",
          quoteSuffix: "",
          fromHint: null,
          toHint: null,
        },
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);
  });

  it("lists a page of 30 ascending comments with a lookahead-derived cursor", async () => {
    const newestFirstRows = Array.from({ length: 31 }, (_, index) =>
      commentRow({
        comment_id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
        body: `Comment ${index}`,
        created_at: new Date(
          Date.parse(timestamp) - index * 1000,
        ).toISOString(),
      }),
    );
    const harness = rpcHarness(newestFirstRows);

    const page = await listSessionShareComments(harness.context, { shareId });

    expect(harness.rpc).toHaveBeenCalledWith("list_session_share_comments", {
      p_share_id: shareId,
      p_before_created_at: null,
      p_before_comment_id: null,
      p_limit: 31,
    });
    expect(page.comments).toHaveLength(30);
    expect(page.comments[0]?.body).toBe("Comment 29");
    expect(page.comments[29]?.body).toBe("Comment 0");
    expect(page.nextCursor).toEqual({
      beforeCreatedAt: new Date(
        Date.parse(timestamp) - 29 * 1000,
      ).toISOString(),
      beforeCommentId: "00000000-0000-4000-8000-000000000029",
    });
  });

  it("omits the cursor on a final page and forwards an explicit cursor", async () => {
    const harness = rpcHarness([anchoredCommentRow(), commentRow()]);

    const page = await listSessionShareComments(harness.context, {
      shareId,
      before: { beforeCreatedAt: timestamp, beforeCommentId: commentId },
    });

    expect(harness.rpc).toHaveBeenCalledWith("list_session_share_comments", {
      p_share_id: shareId,
      p_before_created_at: timestamp,
      p_before_comment_id: commentId,
      p_limit: 31,
    });
    expect(page.nextCursor).toBeNull();
    expect(page.comments.map((comment) => comment.anchor !== null)).toEqual([
      false,
      true,
    ]);
  });

  it("deletes a comment and verifies the identifier round-trip", async () => {
    const harness = rpcHarness([
      { comment_id: commentId, deleted_at: timestamp },
    ]);
    await expect(
      deleteSessionShareComment(harness.context, commentId),
    ).resolves.toEqual({ commentId, deletedAt: timestamp });
    expect(harness.rpc).toHaveBeenCalledWith("delete_session_share_comment", {
      p_comment_id: commentId,
    });

    const mismatched = rpcHarness([
      { comment_id: shareId, deleted_at: timestamp },
    ]);
    await expect(
      deleteSessionShareComment(mismatched.context, commentId),
    ).rejects.toBeInstanceOf(ShareManagementError);
  });
});

describe("session share snapshot publication", () => {
  it("parses raw ProseMirror JSON and publishes before changing access", async () => {
    const events: string[] = [];
    const fetcher = vi.fn(
      async (url: URL | RequestInfo, init?: RequestInit) => {
        events.push("publish");
        expect(url.toString()).toBe(
          `https://api.example.com/sync/shares/${shareId}/snapshot`,
        );
        expect(init).toMatchObject({
          method: "PUT",
          cache: "no-store",
          credentials: "omit",
          redirect: "error",
          referrerPolicy: "no-referrer",
        });
        expect(init?.headers).toMatchObject({
          Authorization: "Bearer authenticated-access-token",
        });
        expect(JSON.parse(String(init?.body))).toEqual({
          baseRevision: 0,
          mutationId,
          title: "Shared title",
          body: {
            type: "doc",
            content: [{ type: "paragraph" }],
          },
        });
        return new Response(
          JSON.stringify({
            shareId,
            schemaVersion: 1,
            contentRevision: 1,
            title: "Shared title",
            body: { type: "doc", content: [{ type: "paragraph" }] },
            attachments: [],
            webEditable: true,
            accessVersion: 1,
            publishedAt: timestamp,
          }),
          { headers: { "content-type": "application/json" } },
        );
      },
    );

    const result = await publishBeforeAccessMutation({
      snapshot: {
        apiBaseUrl: "https://api.example.com/base",
        session: session(),
        shareId,
        baseRevision: 0,
        mutationId,
        title: " Shared title ",
        body: JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph" }],
        }),
        fetcher,
      },
      mutateAccess: async () => {
        events.push("mutate");
        return "changed";
      },
    });

    expect(result).toBe("changed");
    expect(events).toEqual(["publish", "mutate"]);
  });

  it("sends an explicit empty attachment replacement when requested", async () => {
    const fetcher = vi.fn(
      async (_url: URL | RequestInfo, init?: RequestInit) => {
        expect(JSON.parse(String(init?.body))).toEqual({
          baseRevision: 1,
          mutationId,
          title: "Shared title",
          body: { type: "doc", content: [{ type: "paragraph" }] },
          attachmentIds: [],
        });
        return new Response(
          JSON.stringify({
            shareId,
            schemaVersion: 1,
            contentRevision: 2,
            title: "Shared title",
            body: { type: "doc", content: [{ type: "paragraph" }] },
            attachments: [],
            webEditable: true,
            accessVersion: 1,
            publishedAt: timestamp,
          }),
          { headers: { "content-type": "application/json" } },
        );
      },
    );

    await expect(
      publishSessionShareSnapshot({
        apiBaseUrl: "https://api.example.com",
        session: session(),
        shareId,
        baseRevision: 1,
        mutationId,
        title: "Shared title",
        body: { type: "doc", content: [{ type: "paragraph" }] },
        attachmentIds: [],
        fetcher,
      }),
    ).resolves.toMatchObject({ contentRevision: 2, attachments: [] });
  });

  it("does not mutate access when publication fails size validation", async () => {
    const mutateAccess = vi.fn();
    const fetcher = vi.fn(
      async () =>
        new Response("{}", {
          headers: {
            "content-length": String(2 * 1024 * 1024 + 256 * 1024 + 1),
            "content-type": "application/json",
          },
        }),
    );

    await expect(
      publishBeforeAccessMutation({
        snapshot: {
          apiBaseUrl: "https://api.example.com",
          session: session(),
          shareId,
          baseRevision: 1,
          mutationId,
          title: "Title",
          body: { type: "doc" },
          fetcher,
        },
        mutateAccess,
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);
    expect(mutateAccess).not.toHaveBeenCalled();
  });

  it("rejects non-document local JSON before making a request", async () => {
    expect(() => parseSessionShareDocument('{"type":"paragraph"}')).toThrow(
      ShareManagementError,
    );
    const fetcher = vi.fn();
    await expect(
      publishSessionShareSnapshot({
        apiBaseUrl: "https://api.example.com",
        session: session(),
        shareId,
        baseRevision: 1,
        mutationId,
        title: "Title",
        body: "not-json",
        fetcher,
      }),
    ).rejects.toBeInstanceOf(ShareManagementError);
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("surfaces a validated stale-write snapshot without hiding it as a generic failure", async () => {
    const snapshot = {
      shareId,
      schemaVersion: 1,
      contentRevision: 3,
      title: "Latest",
      body: { type: "doc", content: [{ type: "paragraph" }] },
      attachments: [],
      webEditable: true,
      accessVersion: 4,
      publishedAt: timestamp,
    };
    const fetcher = vi.fn(
      async () =>
        new Response(JSON.stringify({ code: "snapshot_conflict", snapshot }), {
          status: 409,
          headers: { "content-type": "application/json" },
        }),
    );

    await expect(
      publishSessionShareSnapshot({
        apiBaseUrl: "https://api.example.com",
        session: session(),
        shareId,
        baseRevision: 2,
        mutationId,
        title: "Stale",
        body: { type: "doc" },
        fetcher,
      }),
    ).rejects.toMatchObject({
      name: "ShareSnapshotConflictError",
      snapshot,
    });
  });
});
