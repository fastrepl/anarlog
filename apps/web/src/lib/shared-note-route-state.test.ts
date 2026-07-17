import assert from "node:assert/strict";
import test from "node:test";

import {
  getInvitationRouteFailure,
  getLinkSharedNoteRouteGate,
} from "./shared-note-route-state.ts";

test("keeps failed invitation acceptance retryable", () => {
  assert.equal(
    getInvitationRouteFailure({
      acceptanceFailed: true,
      inspectionFailed: false,
      inspectionReady: true,
    }),
    "accept-retry",
  );
  assert.equal(
    getInvitationRouteFailure({
      acceptanceFailed: false,
      inspectionFailed: false,
      inspectionReady: true,
    }),
    null,
  );
  assert.equal(
    getInvitationRouteFailure({
      acceptanceFailed: true,
      inspectionFailed: true,
      inspectionReady: false,
    }),
    "unavailable",
  );
});

test("authenticated access outranks continuation failures", () => {
  assert.equal(
    getLinkSharedNoteRouteGate({
      authenticatedNotePending: false,
      continuationFailed: true,
      continuationPending: false,
      hasAuthenticatedNote: true,
      linkSnapshotPending: false,
    }),
    null,
  );
  assert.equal(
    getLinkSharedNoteRouteGate({
      authenticatedNotePending: true,
      continuationFailed: true,
      continuationPending: false,
      hasAuthenticatedNote: false,
      linkSnapshotPending: false,
    }),
    "loading",
  );
  assert.equal(
    getLinkSharedNoteRouteGate({
      authenticatedNotePending: false,
      continuationFailed: true,
      continuationPending: false,
      hasAuthenticatedNote: false,
      linkSnapshotPending: false,
    }),
    "continuation-error",
  );
});
