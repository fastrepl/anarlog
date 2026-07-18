import assert from "node:assert/strict";
import test from "node:test";

import {
  buildAccountShareDeepLink,
  buildShareHandoffDeepLink,
  buildSharedNoteWebPath,
  getSafeSharedNoteHref,
  getSharedNoteDescription,
  parseAuthenticatedSharedNote,
  parseGatewaySharedNote,
  parseSessionAccessRequestState,
  parseSessionInvitationState,
  parseSessionShareAccessEntry,
  parseSessionShareAccessPage,
  parseShareHandoff,
  parseSharedNoteAttachmentDownload,
  parseSharedNoteComment,
  parseSharedNoteCommentPage,
  parseSharedNoteWebEditConflict,
  parseSharedNoteWebEditSnapshot,
  withoutDuplicateLeadingTitle,
} from "./shared-notes.ts";

const BODY = {
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 1 },
      content: [{ type: "text", text: "Weekly sync" }],
    },
    {
      type: "paragraph",
      content: [{ type: "text", text: "Decisions and next steps." }],
    },
  ],
};

test("builds shared-note desktop links with only the supported schemes", () => {
  const shareId = "82a163dd-d595-45f8-8d71-cf38bbb1ce12";
  const requestId = "1b02e758-295d-4ea4-bd0f-6d3f68bcebf6";

  assert.equal(
    buildAccountShareDeepLink(shareId),
    `hyprnote://share/open?mode=account&share_id=${shareId}`,
  );
  assert.equal(
    buildAccountShareDeepLink(shareId, "hyprnote-staging"),
    `hyprnote-staging://share/open?mode=account&share_id=${shareId}`,
  );
  assert.equal(
    buildShareHandoffDeepLink(requestId, "hyprnote-staging"),
    `hyprnote-staging://share/open?mode=handoff&request_id=${requestId}`,
  );
  assert.equal(
    buildShareHandoffDeepLink(requestId, "char" as "hyprnote"),
    `hyprnote://share/open?mode=handoff&request_id=${requestId}`,
  );
});

test("preserves only the staging scheme in shared-note web paths", () => {
  assert.equal(buildSharedNoteWebPath("/share/example/"), "/share/example/");
  assert.equal(
    buildSharedNoteWebPath("/share/example/", "hyprnote-staging"),
    "/share/example/?scheme=hyprnote-staging",
  );
  assert.equal(
    buildSharedNoteWebPath("/share/example/", "char" as "hyprnote"),
    "/share/example/",
  );
});

test("parses the exact public gateway DTO", () => {
  const snapshot = parseGatewaySharedNote({
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 2,
    title: " Weekly sync ",
    body: BODY,
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
  });

  assert.equal(snapshot.title, "Weekly sync");
  assert.equal(snapshot.body.type, "doc");
  assert.throws(() =>
    parseGatewaySharedNote({ snapshot: { shareId: snapshot.shareId } }),
  );
  assert.throws(() =>
    parseGatewaySharedNote({
      ...snapshot,
      accessVersion: 4,
      webEditable: true,
    }),
  );
});

test("parses the exact web-edit snapshot without weakening public reads", () => {
  const response = {
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 3,
    title: "Weekly sync",
    body: BODY,
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
    accessVersion: 8,
    webEditable: true,
  };
  const edited = parseSharedNoteWebEditSnapshot(response);

  assert.equal(edited.snapshot.contentRevision, 3);
  assert.equal(edited.accessVersion, 8);
  assert.equal(edited.webEditable, true);
  assert.throws(() =>
    parseSharedNoteWebEditSnapshot({
      ...edited.snapshot,
      webEditable: true,
    }),
  );
  assert.deepEqual(
    parseSharedNoteWebEditConflict({
      code: "snapshot_conflict",
      snapshot: response,
    }),
    edited,
  );
  assert.throws(() =>
    parseSharedNoteWebEditConflict({
      code: "snapshot_conflict",
      snapshot: response,
      internalReason: "do not expose",
    }),
  );
});

test("maps authenticated snake-case RPC rows without internal identifiers", () => {
  const result = parseAuthenticatedSharedNote({
    share_id: "00000000-0000-4000-8000-000000000001",
    workspace_id: "private-workspace",
    session_id: "private-session",
    schema_version: 1,
    content_revision: 3,
    title: "Weekly sync",
    body_json: BODY,
    attachments_json: [],
    capability: "commenter",
    manage_access: true,
    access_version: 7,
    web_editable: false,
    published_at: "2026-07-17T12:00:00Z",
  });

  assert.equal(result.capability, "commenter");
  assert.equal(result.manageAccess, true);
  assert.equal(result.accessVersion, 7);
  assert.equal(result.webEditable, false);
  assert.equal("workspaceId" in result.snapshot, false);
  assert.equal("sessionId" in result.snapshot, false);
  assert.throws(() =>
    parseAuthenticatedSharedNote({
      share_id: "00000000-0000-4000-8000-000000000001",
      schema_version: 1,
      content_revision: 3,
      title: "Weekly sync",
      body_json: BODY,
      capability: "commenter",
      published_at: "2026-07-17T12:00:00Z",
    }),
  );

  assert.throws(() =>
    parseAuthenticatedSharedNote({
      share_id: "00000000-0000-4000-8000-000000000001",
      schema_version: 1,
      content_revision: 3,
      title: "Weekly sync",
      body_json: BODY,
      capability: "commenter",
      manage_access: true,
      access_version: 0,
      web_editable: false,
      published_at: "2026-07-17T12:00:00Z",
    }),
  );
});

test("parses bounded shared-note comment rows", () => {
  assert.deepEqual(
    parseSharedNoteComment({
      comment_id: "00000000-0000-4000-8000-000000000002",
      is_author: true,
      body: "Looks good to me.",
      snapshot_content_revision: 4,
      created_at: "2026-07-17T12:00:00Z",
    }),
    {
      commentId: "00000000-0000-4000-8000-000000000002",
      isAuthor: true,
      body: "Looks good to me.",
      snapshotRevision: 4,
      createdAt: "2026-07-17T12:00:00Z",
    },
  );

  const valid = {
    comment_id: "00000000-0000-4000-8000-000000000002",
    is_author: false,
    body: "Looks good to me.",
    snapshot_content_revision: 4,
    created_at: "2026-07-17T12:00:00Z",
  };
  assert.throws(() => parseSharedNoteComment({ ...valid, body: "" }));
  assert.throws(() =>
    parseSharedNoteComment({ ...valid, body: "a".repeat(16385) }),
  );
  assert.throws(() => parseSharedNoteComment({ ...valid, is_author: "yes" }));
  assert.throws(() =>
    parseSharedNoteComment({ ...valid, snapshot_content_revision: 0 }),
  );
  assert.throws(() => parseSharedNoteComment({ ...valid, private_note: true }));
});

test("bounds comment pages and derives an older-history cursor", () => {
  const rows = Array.from({ length: 101 }, (_, index) => ({
    comment_id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
    is_author: index === 0,
    body: `Comment ${index}`,
    snapshot_content_revision: 4,
    created_at: new Date(
      Date.UTC(2026, 6, 17, 12, 0, 101 - index),
    ).toISOString(),
  }));

  const page = parseSharedNoteCommentPage(rows);
  assert.equal(page.comments.length, 100);
  assert.equal(page.comments[0]?.commentId, rows[99]?.comment_id);
  assert.equal(page.comments.at(-1)?.commentId, rows[0]?.comment_id);
  assert.deepEqual(page.nextCursor, {
    beforeCreatedAt: rows[99]?.created_at,
    beforeCommentId: rows[99]?.comment_id,
  });
  assert.equal(
    page.comments.some(
      (comment) => comment.commentId === rows[100]?.comment_id,
    ),
    false,
  );
  assert.equal(parseSharedNoteCommentPage(rows.slice(0, 100)).nextCursor, null);
  assert.throws(() => parseSharedNoteCommentPage([...rows, rows[0]]));
});

test("parses access request state and enforces reviewed-state invariants", () => {
  const pending = {
    request_id: "00000000-0000-4000-8000-000000000003",
    requested_capability: "commenter",
    status: "pending",
    created_at: "2026-07-17T12:00:00Z",
    reviewed_at: null,
  };
  assert.deepEqual(parseSessionAccessRequestState(pending), {
    requestId: pending.request_id,
    requestedCapability: "commenter",
    status: "pending",
    createdAt: pending.created_at,
    reviewedAt: null,
  });
  assert.throws(() =>
    parseSessionAccessRequestState({
      ...pending,
      status: "approved",
      reviewed_at: null,
    }),
  );
  assert.throws(() =>
    parseSessionAccessRequestState({
      ...pending,
      reviewed_at: "2026-07-17T12:01:00Z",
    }),
  );
  assert.throws(() =>
    parseSessionAccessRequestState({ ...pending, status: "unknown" }),
  );
});

test("parses invitation states without exposing unrelated fields", () => {
  assert.deepEqual(
    parseSessionInvitationState({
      status: "accepted",
      capability: "editor",
      share_id: "00000000-0000-4000-8000-000000000001",
    }),
    {
      status: "accepted",
      capability: "editor",
      shareId: "00000000-0000-4000-8000-000000000001",
    },
  );
  assert.throws(() =>
    parseSessionInvitationState({
      status: "accepted",
      capability: "owner",
      share_id: null,
    }),
  );
  assert.throws(() =>
    parseSessionInvitationState({
      status: "pending",
      capability: "viewer",
      share_id: null,
      token_hash: "private",
    }),
  );
});

test("parses only valid manager access entry variants", () => {
  const grant = {
    entry_type: "grant",
    entry_id: "00000000-0000-4000-8000-000000000006",
    user_id: "00000000-0000-4000-8000-000000000007",
    user_email: "grantee@example.com",
    capability: "editor",
    status: "active",
    created_at: "2026-07-17T12:00:00Z",
    expires_at: null,
  };
  const invitation = {
    entry_type: "invitation",
    entry_id: "00000000-0000-4000-8000-000000000004",
    user_id: null,
    user_email: "ada@example.com",
    capability: "commenter",
    status: "pending",
    created_at: "2026-07-17T12:00:00Z",
    expires_at: "2026-07-24T12:00:00Z",
  };
  const request = {
    entry_type: "request",
    entry_id: "00000000-0000-4000-8000-000000000008",
    user_id: "00000000-0000-4000-8000-000000000009",
    user_email: "requester@example.com",
    capability: "viewer",
    status: "pending",
    created_at: "2026-07-17T12:00:00Z",
    expires_at: null,
  };
  assert.equal(parseSessionShareAccessEntry(grant).entryType, "grant");
  assert.deepEqual(parseSessionShareAccessEntry(invitation), {
    entryType: "invitation",
    entryId: invitation.entry_id,
    userId: null,
    userEmail: "ada@example.com",
    capability: "commenter",
    status: "pending",
    createdAt: invitation.created_at,
    expiresAt: invitation.expires_at,
  });
  assert.deepEqual(parseSessionShareAccessEntry(request), {
    entryType: "request",
    entryId: request.entry_id,
    userId: request.user_id,
    userEmail: request.user_email,
    capability: "viewer",
    status: "pending",
    createdAt: request.created_at,
    expiresAt: null,
  });

  assert.throws(() =>
    parseSessionShareAccessEntry({
      ...invitation,
      status: "active",
    }),
  );
  assert.throws(() =>
    parseSessionShareAccessEntry({
      ...invitation,
      user_email: "Ada@Example.com",
    }),
  );
  assert.throws(() =>
    parseSessionShareAccessEntry({
      ...invitation,
      expires_at: null,
    }),
  );
  assert.throws(() =>
    parseSessionShareAccessEntry({
      ...invitation,
      secret: "private",
    }),
  );
  assert.throws(() =>
    parseSessionShareAccessEntry({
      ...request,
      user_email: null,
    }),
  );
});

test("bounds manager access pages and derives the descending cursor", () => {
  const rows = Array.from({ length: 101 }, (_, index) => ({
    entry_type: "request",
    entry_id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
    user_id: `00000000-0000-4001-8000-${String(index).padStart(12, "0")}`,
    user_email: `requester-${index}@example.com`,
    capability: "commenter",
    status: "pending",
    created_at: new Date(
      Date.UTC(2026, 6, 17, 12, 0, 101 - index),
    ).toISOString(),
    expires_at: null,
  }));

  const page = parseSessionShareAccessPage(rows);
  assert.equal(page.entries.length, 100);
  assert.deepEqual(page.nextCursor, {
    beforeCreatedAt: rows[99]?.created_at,
    beforeEntryId: rows[99]?.entry_id,
  });
  assert.equal(
    page.entries.some((entry) => entry.entryId === rows[100]?.entry_id),
    false,
  );
  assert.equal(
    parseSessionShareAccessPage(rows.slice(0, 100)).nextCursor,
    null,
  );
  assert.throws(() => parseSessionShareAccessPage([...rows, rows[0]]));
});

test("accepts a bounded opaque attachment manifest and rejects duplicates", () => {
  const attachment = {
    id: "00000000-0000-4000-8000-000000000002",
    filename: "diagram.png",
    contentType: "image/png",
    sizeBytes: 4,
    sha256: "a".repeat(64),
  };
  const snapshot = parseGatewaySharedNote({
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 2,
    title: "Weekly sync",
    body: {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: { sharedAttachmentId: attachment.id, alt: "Diagram" },
        },
      ],
    },
    attachments: [attachment],
    publishedAt: "2026-07-17T12:00:00Z",
  });
  assert.deepEqual(snapshot.attachments, [attachment]);
  assert.throws(() =>
    parseGatewaySharedNote({
      ...snapshot,
      attachments: [attachment, attachment],
    }),
  );
});

test("validates short-lived attachment downloads", () => {
  const download = parseSharedNoteAttachmentDownload({
    id: "00000000-0000-4000-8000-000000000002",
    filename: "diagram.png",
    contentType: "image/png",
    sizeBytes: 4,
    sha256: "a".repeat(64),
    signedUrl:
      "https://project.supabase.co/storage/v1/object/sign/shared-note-attachments/file?token=secret",
    expiresAt: "2026-07-17T12:01:00Z",
  });
  assert.equal(download.filename, "diagram.png");
  assert.throws(() =>
    parseSharedNoteAttachmentDownload({
      ...download,
      signedUrl: "http://project.supabase.co/file?token=secret",
    }),
  );
});

test("rejects unsafe links and accepts sanitized external links", () => {
  assert.equal(getSafeSharedNoteHref("javascript:alert(1)"), null);
  assert.equal(getSafeSharedNoteHref("file:///tmp/private"), null);
  assert.equal(getSafeSharedNoteHref("https://user:pass@example.com"), null);
  assert.equal(
    getSafeSharedNoteHref("https://example.com/note?q=1"),
    "https://example.com/note?q=1",
  );
});

test("builds bounded descriptions and removes a duplicate title heading", () => {
  const snapshot = parseGatewaySharedNote({
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 1,
    title: "Weekly sync",
    body: BODY,
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
  });
  const body = withoutDuplicateLeadingTitle(snapshot.body, snapshot.title);

  assert.equal(body.content?.[0]?.type, "paragraph");
  assert.match(getSharedNoteDescription(snapshot.body), /Decisions/);
  assert.ok(getSharedNoteDescription(snapshot.body).length <= 180);
});

test("parses handoff responses and rejects invalid timestamps", () => {
  assert.deepEqual(
    parseShareHandoff({
      requestId: "00000000-0000-4000-8000-000000000001",
      expiresAt: "2026-07-17T12:01:00Z",
    }),
    {
      requestId: "00000000-0000-4000-8000-000000000001",
      expiresAt: "2026-07-17T12:01:00Z",
    },
  );
  assert.throws(() =>
    parseShareHandoff({
      requestId: "00000000-0000-4000-8000-000000000001",
      expiresAt: "later",
    }),
  );
});
