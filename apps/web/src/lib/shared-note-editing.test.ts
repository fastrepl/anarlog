import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSharedNoteWebEditInput,
  canEditSharedNoteOnWeb,
  deriveSharedNoteEditorTitle,
  ensureSharedNoteEditorTitle,
  getSharedNoteWebEditPreparationMessage,
  hasUnsupportedSharedNoteEditorNode,
  reuseSharedNoteMutationIdForUnchangedDraft,
} from "./shared-note-editing.ts";
import type { SharedNoteDocument, SharedNoteSnapshot } from "./shared-notes.ts";

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

test("blocks attachment nodes from the v1 web editor at any depth", () => {
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
          ],
        },
      ],
    }),
    true,
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
