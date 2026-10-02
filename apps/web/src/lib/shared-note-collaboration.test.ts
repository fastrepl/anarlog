import assert from "node:assert/strict";
import test from "node:test";

import {
  canComposeSharedNoteComments,
  hasSharedNoteCollaborationAccess,
  selectSharedNoteCommentAccessRequest,
  MAX_SHARED_NOTE_COMMENT_ANCHOR_CONTEXT_BYTES,
  MAX_SHARED_NOTE_COMMENT_ANCHOR_EXACT_BYTES,
  MAX_SHARED_NOTE_COMMENT_BYTES,
  validateSharedNoteCommentAnchor,
  validateSharedNoteCommentBody,
} from "./shared-note-collaboration.ts";

test("comment anchors validate byte caps without trimming", () => {
  const anchor = {
    quoteExact: "  quoted text  ",
    quotePrefix: "before ",
    quoteSuffix: " after",
    fromHint: 2,
    toHint: 17,
  };
  const validated = validateSharedNoteCommentAnchor(anchor);
  assert.equal(validated.valid, true);
  assert.equal(validated.anchor?.quoteExact, "  quoted text  ");

  assert.equal(validateSharedNoteCommentAnchor(null).valid, true);
  assert.equal(validateSharedNoteCommentAnchor(null).anchor, null);
  assert.equal(validateSharedNoteCommentAnchor(undefined).valid, true);

  const cjk = "ㄱ".repeat(
    Math.floor(MAX_SHARED_NOTE_COMMENT_ANCHOR_EXACT_BYTES / 3),
  );
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, quoteExact: cjk }).valid,
    true,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, quoteExact: `${cjk}xx` })
      .valid,
    false,
  );

  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, quoteExact: "" }).valid,
    false,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({
      ...anchor,
      quotePrefix: "p".repeat(MAX_SHARED_NOTE_COMMENT_ANCHOR_CONTEXT_BYTES + 1),
    }).valid,
    false,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({
      ...anchor,
      quoteSuffix: "s".repeat(MAX_SHARED_NOTE_COMMENT_ANCHOR_CONTEXT_BYTES + 1),
    }).valid,
    false,
  );
});

test("comment anchor hints must be paired and ordered", () => {
  const anchor = {
    quoteExact: "quoted",
    quotePrefix: "",
    quoteSuffix: "",
    fromHint: null,
    toHint: null,
  };
  assert.equal(validateSharedNoteCommentAnchor(anchor).valid, true);
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, fromHint: 1, toHint: 5 })
      .valid,
    true,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, fromHint: 1 }).valid,
    false,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, toHint: 5 }).valid,
    false,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, fromHint: 0, toHint: 5 })
      .valid,
    false,
  );
  assert.equal(
    validateSharedNoteCommentAnchor({ ...anchor, fromHint: 5, toHint: 5 })
      .valid,
    false,
  );
});

test("only named commenters, editors, and managers can compose comments", () => {
  assert.equal(
    canComposeSharedNoteComments({
      capability: "commenter",
      hasCollaborationAccess: false,
      manageAccess: false,
    }),
    false,
  );
  assert.equal(
    canComposeSharedNoteComments({
      capability: "viewer",
      hasCollaborationAccess: true,
      manageAccess: false,
    }),
    false,
  );
  for (const capability of ["commenter", "editor"] as const) {
    assert.equal(
      canComposeSharedNoteComments({
        capability,
        hasCollaborationAccess: true,
        manageAccess: false,
      }),
      true,
    );
  }
  assert.equal(
    canComposeSharedNoteComments({
      capability: "viewer",
      hasCollaborationAccess: true,
      manageAccess: true,
    }),
    true,
  );
});

test("only an authorized comment feed grants collaboration access", () => {
  assert.equal(hasSharedNoteCollaborationAccess({ status: "ready" }), true);
  assert.equal(
    hasSharedNoteCollaborationAccess({ status: "unavailable" }),
    false,
  );
  assert.equal(hasSharedNoteCollaborationAccess({ status: "error" }), false);
  assert.equal(hasSharedNoteCollaborationAccess(undefined), false);
});

test("comment validation uses the normalized UTF-8 byte length", () => {
  assert.deepEqual(validateSharedNoteCommentBody(" \n\t "), {
    body: "",
    byteLength: 0,
    valid: false,
  });
  assert.deepEqual(validateSharedNoteCommentBody("  hello  "), {
    body: "hello",
    byteLength: 5,
    valid: true,
  });
  assert.equal(
    validateSharedNoteCommentBody("x".repeat(MAX_SHARED_NOTE_COMMENT_BYTES))
      .valid,
    true,
  );
  assert.equal(
    validateSharedNoteCommentBody("x".repeat(MAX_SHARED_NOTE_COMMENT_BYTES + 1))
      .valid,
    false,
  );
  assert.deepEqual(validateSharedNoteCommentBody("字".repeat(5_461)), {
    body: "字".repeat(5_461),
    byteLength: 16_383,
    valid: true,
  });
  assert.equal(validateSharedNoteCommentBody("字".repeat(5_462)).valid, false);
});

test("comment panel ignores viewer requests made from the access gate", () => {
  const base = {
    requestId: "00000000-0000-4000-8000-000000000001",
    createdAt: "2026-09-09T09:00:00Z",
    reviewedAt: "2026-09-09T09:05:00Z",
  } as const;
  assert.equal(
    selectSharedNoteCommentAccessRequest({
      ...base,
      requestedCapability: "viewer",
      status: "approved",
    }),
    null,
  );
  const commentRequest = {
    ...base,
    requestedCapability: "commenter",
    status: "pending",
    reviewedAt: null,
  } as const;
  assert.equal(
    selectSharedNoteCommentAccessRequest(commentRequest),
    commentRequest,
  );
  assert.equal(selectSharedNoteCommentAccessRequest(null), null);
});
