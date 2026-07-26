import type { SharedNoteSnapshot } from "./shared-notes";

export function getInvitationRouteFailure({
  acceptanceFailed,
  inspectionFailed,
  inspectionReady,
}: {
  acceptanceFailed: boolean;
  inspectionFailed: boolean;
  inspectionReady: boolean;
}): "accept-retry" | "unavailable" | null {
  if (inspectionFailed || !inspectionReady) {
    return "unavailable";
  }
  return acceptanceFailed ? "accept-retry" : null;
}

export function getLinkSharedNoteRouteGate({
  authenticatedNotePending,
  continuationFailed,
  continuationPending,
  hasAuthenticatedNote,
  linkSnapshotPending,
}: {
  authenticatedNotePending: boolean;
  continuationFailed: boolean;
  continuationPending: boolean;
  hasAuthenticatedNote: boolean;
  linkSnapshotPending: boolean;
}): "continuation-error" | "loading" | null {
  if (hasAuthenticatedNote) {
    return null;
  }
  if (authenticatedNotePending) {
    return "loading";
  }
  if (continuationFailed) {
    return "continuation-error";
  }
  if (continuationPending || linkSnapshotPending) {
    return "loading";
  }
  return null;
}

export function getLinkSharedNoteFallbackSnapshot({
  authenticatedSnapshot,
  linkSnapshot,
}: {
  authenticatedSnapshot: SharedNoteSnapshot | null;
  linkSnapshot: SharedNoteSnapshot | null;
}): SharedNoteSnapshot | null {
  return linkSnapshot ?? authenticatedSnapshot;
}
