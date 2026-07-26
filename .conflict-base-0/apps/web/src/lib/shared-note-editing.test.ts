import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSharedNoteWebEditInput,
  canEditSharedNoteOnWeb,
  canonicalizeSharedNoteWebDraft,
  deriveSharedNoteEditorTitle,
  ensureSharedNoteEditorTitle,
  getSharedNoteReadOnlySnapshot,
  getSharedNoteWebEditPreparationMessage,
  hasUnsupportedSharedNoteEditorNode,
  resolveSharedNoteViewerAuthorization,
  reuseSharedNoteMutationIdForUnchangedDraft,
  shouldRenderSharedNoteUnavailable,
  syncSharedNoteViewerAuthorization,
} from "./shared-note-editing.ts";
import type {
  AuthenticatedSharedNote,
  SharedNoteDocument,
  SharedNoteSnapshot,
} from "./shared-notes.ts";

const BODY: SharedNoteDocument = {
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 1 },
      content: [{ type: "text", text: "  Weekly sync  " }],
    },
    {
      type: "paragraph",
      content: [{ type: "text", text: "Decisions and next steps." }],
    },
  ],
};

test("allows web editing only for an authenticated editable editor", () => {
  assert.equal(canEditSharedNoteOnWeb(null), false);
  assert.equal(
    canEditSharedNoteOnWeb({ capability: "viewer", webEditable: true }),
    false,
  );
  assert.equal(
    canEditSharedNoteOnWeb({ capability: "editor", webEditable: false }),
    false,
  );
  assert.equal(
    canEditSharedNoteOnWeb({ capability: "editor", webEditable: true }),
    true,
  );
});

test("keeps sign-in expiry sticky across stale authenticated prop updates", () => {
  const expired = {
    note: null,
    state: "sign_in_required" as const,
  };
  const staleAuthenticatedNote = authenticatedEditor(3);

  assert.equal(
    syncSharedNoteViewerAuthorization(expired, staleAuthenticatedNote),
    expired,
  );
});

test("restores confirmed edit access without requiring a version increase", () => {
  const refreshed = authenticatedEditor(3);

  assert.deepEqual(resolveSharedNoteViewerAuthorization(refreshed), {
    note: refreshed,
    state: "ready",
  });
  assert.equal(
    resolveSharedNoteViewerAuthorization({
      ...refreshed,
      capability: "viewer",
    }).state,
    "access_changed",
  );
  assert.equal(
    resolveSharedNoteViewerAuthorization({
      ...refreshed,
      webEditable: false,
    }).state,
    "ready",
  );
});

test("shows the preparation message only to authenticated editors", () => {
  assert.equal(
    getSharedNoteWebEditPreparationMessage(
      { capability: "viewer", webEditable: false },
      false,
    ),
    null,
  );
  assert.match(
    getSharedNoteWebEditPreparationMessage(
      { capability: "editor", webEditable: false },
      false,
    ) ?? "",
    /needs to be prepared/i,
  );
  assert.match(
    getSharedNoteWebEditPreparationMessage(
      { capability: "editor", webEditable: true },
      true,
    ) ?? "",
    /needs to be prepared/i,
  );
  assert.equal(
    getSharedNoteWebEditPreparationMessage(
      { capability: "editor", webEditable: true },
      false,
    ),
    null,
  );
});

test("keeps a read-only fallback visible while changed access is refreshed", () => {
  assert.equal(
    shouldRenderSharedNoteUnavailable({
      accessRevoked: true,
      hasFallbackSnapshot: true,
      revokedBehavior: "read-only",
    }),
    false,
  );
  assert.equal(
    shouldRenderSharedNoteUnavailable({
      accessRevoked: true,
      hasFallbackSnapshot: false,
      revokedBehavior: "read-only",
    }),
    true,
  );
  assert.equal(
    shouldRenderSharedNoteUnavailable({
      accessRevoked: true,
      hasFallbackSnapshot: true,
      revokedBehavior: "unavailable",
    }),
    true,
  );
});

test("keeps the newest same-note snapshot when editing access changes", () => {
  const current = sharedNoteSnapshot(8);
  const staleFallback = sharedNoteSnapshot(7);
  const newerFallback = sharedNoteSnapshot(9);

  assert.equal(getSharedNoteReadOnlySnapshot(current, staleFallback), current);
  assert.equal(
    getSharedNoteReadOnlySnapshot(current, newerFallback),
    newerFallback,
  );
  assert.equal(
    getSharedNoteReadOnlySnapshot(current, {
      ...newerFallback,
      shareId: "00000000-0000-4000-8000-000000000099",
    }),
    null,
  );
});

test("keeps or restores the canonical leading title heading", () => {
  assert.equal(ensureSharedNoteEditorTitle(BODY, "Weekly sync"), BODY);

  const bodyWithoutTitle: SharedNoteDocument = {
    type: "doc",
    content: [{ type: "paragraph" }],
  };
  const prepared = ensureSharedNoteEditorTitle(bodyWithoutTitle, "Weekly sync");
  assert.deepEqual(prepared.content?.[0], {
    type: "heading",
    attrs: { level: 1 },
    content: [{ type: "text", text: "Weekly sync" }],
  });
  assert.equal(prepared.content?.[1], bodyWithoutTitle.content?.[0]);
});

function sharedNoteSnapshot(contentRevision: number): SharedNoteSnapshot {
  return {
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision,
    title: "Weekly sync",
    body: BODY,
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
  };
}

function authenticatedEditor(accessVersion: number): AuthenticatedSharedNote {
  return {
    snapshot: sharedNoteSnapshot(7),
    capability: "editor",
    manageAccess: false,
    accessVersion,
    webEditable: true,
  };
}

test("builds a revision-safe payload from the live canonical document", () => {
  const snapshot: SharedNoteSnapshot = {
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 7,
    title: "Old title",
    body: BODY,
    attachments: [
      {
        id: "00000000-0000-4000-8000-000000000003",
        filename: "first.txt",
        contentType: "text/plain",
        sizeBytes: 1,
        sha256: "a".repeat(64),
      },
      {
        id: "00000000-0000-4000-8000-000000000002",
        filename: "second.txt",
        contentType: "text/plain",
        sizeBytes: 1,
        sha256: "b".repeat(64),
      },
    ],
    publishedAt: "2026-07-17T12:00:00Z",
  };
  const mutationId = "00000000-0000-4000-8000-000000000004";
  const input = buildSharedNoteWebEditInput({
    body: BODY,
    mutationId,
    snapshot,
  });

  assert.equal(deriveSharedNoteEditorTitle(BODY), "Weekly sync");
  assert.equal(input.baseRevision, 7);
  assert.equal(input.mutationId, mutationId);
  assert.equal(input.title, "Weekly sync");
  assert.equal(input.body, BODY);
  assert.deepEqual(input.attachmentIds, [
    "00000000-0000-4000-8000-000000000003",
    "00000000-0000-4000-8000-000000000002",
  ]);
});

test("allows shared attachments but keeps clips blocked at any depth", () => {
  assert.equal(hasUnsupportedSharedNoteEditorNode(BODY), false);
  assert.equal(
    hasUnsupportedSharedNoteEditorNode({
      type: "doc",
      content: [
        {
          type: "blockquote",
          content: [
            {
              type: "image",
              attrs: { sharedAttachmentId: "attachment" },
            },
            {
              type: "fileAttachment",
              attrs: { sharedAttachmentId: "attachment" },
            },
          ],
        },
      ],
    }),
    false,
  );
  assert.equal(
    hasUnsupportedSharedNoteEditorNode({
      type: "doc",
      content: [
        {
          type: "blockquote",
          content: [{ type: "clip" }],
        },
      ],
    }),
    true,
  );
});

test("canonicalizes nested attachment nodes without mutating the draft", () => {
  const draft: SharedNoteDocument = {
    type: "doc",
    attrs: { source: "web" },
    content: [
      {
        type: "blockquote",
        attrs: { cite: "https://example.com" },
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Diagram" }],
          },
          {
            type: "image",
            attrs: {
              sharedAttachmentId: "attachment-image",
              src: "blob:private-image",
              alt: "Diagram",
              editorWidth: 640,
            },
            marks: [{ type: "link", attrs: { href: "https://example.com" } }],
          },
          {
            type: "fileAttachment",
            attrs: {
              sharedAttachmentId: "attachment-file",
              src: "blob:private-file",
              path: "/private/report.pdf",
              filename: "report.pdf",
            },
          },
        ],
      },
    ],
  };
  const original = structuredClone(draft);

  assert.deepEqual(
    canonicalizeSharedNoteWebDraft(draft, [
      "attachment-image",
      "attachment-file",
    ]),
    {
      type: "doc",
      attrs: { source: "web" },
      content: [
        {
          type: "blockquote",
          attrs: { cite: "https://example.com" },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "Diagram" }],
            },
            {
              type: "image",
              attrs: { sharedAttachmentId: "attachment-image" },
            },
            {
              type: "fileAttachment",
              attrs: { sharedAttachmentId: "attachment-file" },
            },
          ],
        },
      ],
    },
  );
  assert.deepEqual(draft, original);
});

test("rejects attachment nodes with missing or foreign manifest IDs", () => {
  assert.equal(
    canonicalizeSharedNoteWebDraft(
      {
        type: "doc",
        content: [{ type: "image", attrs: { src: "blob:private" } }],
      },
      ["attachment-image"],
    ),
    null,
  );
  assert.equal(
    canonicalizeSharedNoteWebDraft(
      {
        type: "doc",
        content: [
          {
            type: "fileAttachment",
            attrs: { sharedAttachmentId: "attachment-foreign" },
          },
        ],
      },
      ["attachment-file"],
    ),
    null,
  );
  assert.equal(
    canonicalizeSharedNoteWebDraft(
      { type: "doc", content: [{ type: "clip", attrs: { src: "private" } }] },
      [],
    ),
    null,
  );
});

test("reuses a failed mutation only while the live draft is unchanged", () => {
  const snapshot: SharedNoteSnapshot = {
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 7,
    title: "Weekly sync",
    body: BODY,
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
  };
  const previous = buildSharedNoteWebEditInput({
    body: BODY,
    mutationId: "00000000-0000-4000-8000-000000000004",
    snapshot,
  });
  const unchanged = buildSharedNoteWebEditInput({
    body: structuredClone(BODY),
    mutationId: "00000000-0000-4000-8000-000000000005",
    snapshot,
  });
  const changed = buildSharedNoteWebEditInput({
    body: {
      ...BODY,
      content: [
        ...(BODY.content ?? []),
        { type: "paragraph", content: [{ type: "text", text: "New" }] },
      ],
    },
    mutationId: "00000000-0000-4000-8000-000000000006",
    snapshot,
  });

  assert.equal(
    reuseSharedNoteMutationIdForUnchangedDraft(unchanged, previous).mutationId,
    previous.mutationId,
  );
  assert.equal(
    reuseSharedNoteMutationIdForUnchangedDraft(changed, previous).mutationId,
    changed.mutationId,
  );
});
