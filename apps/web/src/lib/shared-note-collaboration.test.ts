import assert from "node:assert/strict";
import test from "node:test";

import {
  canComposeSharedNoteComments,
  formatAuthenticatedSharedNoteAccessLabel,
  formatSharedNoteAccessRequestDescription,
  hasSharedNoteCollaborationAccess,
  MAX_SHARED_NOTE_COMMENT_BYTES,
  shouldUseAuthenticatedSharedNoteAccessLabel,
  validateSharedNoteCommentBody,
} from "./shared-note-collaboration.ts";

test("general viewer access never implies comment access", () => {
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
});

test("named commenters, editors, and managers can compose", () => {
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

test("access labels match the authenticated capability", () => {
  assert.equal(
    formatAuthenticatedSharedNoteAccessLabel({
      capability: "viewer",
      manageAccess: false,
    }),
    "Shared with you · View only",
  );
  assert.equal(
    formatAuthenticatedSharedNoteAccessLabel({
      capability: "commenter",
      manageAccess: false,
    }),
    "Shared with you · Can comment",
  );
  assert.equal(
    formatAuthenticatedSharedNoteAccessLabel({
      capability: "editor",
      manageAccess: true,
    }),
    "You manage this note · Can edit and comment",
  );
});

test("general viewers keep the route-level access label", () => {
  assert.equal(
    shouldUseAuthenticatedSharedNoteAccessLabel({
      capability: "viewer",
      manageAccess: false,
    }),
    false,
  );
  assert.equal(
    shouldUseAuthenticatedSharedNoteAccessLabel({
      capability: "commenter",
      manageAccess: false,
    }),
    true,
  );
  assert.equal(
    shouldUseAuthenticatedSharedNoteAccessLabel({
      capability: "editor",
      manageAccess: false,
    }),
    true,
  );
  assert.equal(
    shouldUseAuthenticatedSharedNoteAccessLabel({
      capability: "viewer",
      manageAccess: true,
    }),
    true,
  );
});

test("access request descriptions preserve the requested capability", () => {
  assert.equal(
    formatSharedNoteAccessRequestDescription("viewer"),
    "Requested permission to view",
  );
  assert.equal(
    formatSharedNoteAccessRequestDescription("commenter"),
    "Requested permission to comment",
  );
  assert.equal(
    formatSharedNoteAccessRequestDescription("editor"),
    "Requested permission to edit",
  );
});
