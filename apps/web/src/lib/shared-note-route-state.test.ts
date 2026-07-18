import assert from "node:assert/strict";
import test from "node:test";

import {
  getInvitationRouteFailure,
  getLinkSharedNoteFallbackSnapshot,
  getLinkSharedNoteRouteGate,
} from "./shared-note-route-state.ts";
import type { SharedNoteSnapshot } from "./shared-notes.ts";

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

test("link access provides an authenticated fallback without a token", () => {
  const authenticatedSnapshot = sharedNoteSnapshot(7);
  const linkSnapshot = sharedNoteSnapshot(6);

  assert.equal(
    getLinkSharedNoteFallbackSnapshot({
      authenticatedSnapshot,
      linkSnapshot: null,
    }),
    authenticatedSnapshot,
  );
  assert.equal(
    getLinkSharedNoteFallbackSnapshot({
      authenticatedSnapshot,
      linkSnapshot,
    }),
    linkSnapshot,
  );
});

function sharedNoteSnapshot(contentRevision: number): SharedNoteSnapshot {
  return {
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision,
    title: "Weekly sync",
    body: { type: "doc" },
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
  };
}
