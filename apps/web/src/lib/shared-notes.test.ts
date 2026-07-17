import assert from "node:assert/strict";
import test from "node:test";

import {
  getSafeSharedNoteHref,
  getSharedNoteDescription,
  parseAuthenticatedSharedNote,
  parseGatewaySharedNote,
  parseShareHandoff,
  parseSharedNoteAttachmentDownload,
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
    published_at: "2026-07-17T12:00:00Z",
  });

  assert.equal(result.capability, "commenter");
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
