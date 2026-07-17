import assert from "node:assert/strict";
import test from "node:test";

import {
  canComposeSharedNoteComments,
  formatAuthenticatedSharedNoteAccessLabel,
} from "./shared-note-collaboration.ts";

test("general viewer access never implies comment access", () => {
  assert.equal(
    canComposeSharedNoteComments({
      capability: "commenter",
      hasAuthenticatedAccess: false,
      manageAccess: false,
    }),
    false,
  );
  assert.equal(
    canComposeSharedNoteComments({
      capability: "viewer",
      hasAuthenticatedAccess: true,
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
        hasAuthenticatedAccess: true,
        manageAccess: false,
      }),
      true,
    );
  }
  assert.equal(
    canComposeSharedNoteComments({
      capability: "viewer",
      hasAuthenticatedAccess: true,
      manageAccess: true,
    }),
    true,
  );
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
