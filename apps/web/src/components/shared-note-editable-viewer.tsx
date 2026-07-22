import { useState } from "react";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import { SharedNoteReader } from "@/components/shared-note-reader";
import {
  SharedNoteUnavailable,
  SharedNoteViewer,
} from "@/components/shared-note-viewer";
import { canComposeSharedNoteComments } from "@/lib/shared-note-collaboration";
import {
  getSharedNoteReadOnlySnapshot,
  shouldRenderSharedNoteUnavailable,
  syncSharedNoteViewerAuthorization,
  type SharedNoteViewerAuthorization,
} from "@/lib/shared-note-editing";
import type {
  AuthenticatedSharedNote,
  SharedNoteSnapshot,
} from "@/lib/shared-notes";

export function SharedNoteEditableViewer({
  accessLabel,
  actions,
  authenticatedNote,
  chat,
  fallbackAccessLabel,
  fallbackSnapshot,
  resolveAttachment,
  revokedBehavior,
  signedIn,
  snapshot: initialSnapshot,
}: {
  accessLabel: string;
  actions?: React.ReactNode;
  authenticatedNote: AuthenticatedSharedNote | null;
  // Render slot for the chat panel; receives the live snapshot so the chat
  // always answers about the content currently on screen, and lives inside
  // this shareId-keyed subtree so its state resets on navigation.
  chat?: (snapshot: SharedNoteSnapshot) => React.ReactNode;
  fallbackAccessLabel?: string;
  fallbackSnapshot?: SharedNoteSnapshot | null;
  resolveAttachment?: SharedAttachmentResolver;
  revokedBehavior: "read-only" | "unavailable";
  signedIn: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [authorization, setAuthorization] =
    useState<SharedNoteViewerAuthorization>({
      note: authenticatedNote,
      state: "ready",
    });
  const [source, setSource] = useState({
    authenticatedNote,
    snapshot: initialSnapshot,
  });
  if (
    source.snapshot !== initialSnapshot ||
    source.authenticatedNote !== authenticatedNote
  ) {
    setSource({ authenticatedNote, snapshot: initialSnapshot });
    setSnapshot((current) =>
      initialSnapshot.contentRevision >= current.contentRevision
        ? initialSnapshot
        : current,
    );
    if (authorization.state !== "sign_in_required") {
      setAuthorization((current) =>
        syncSharedNoteViewerAuthorization(current, authenticatedNote),
      );
    }
  }

  const accessRevoked = authorization.state === "access_changed";
  const requiresSignIn = authorization.state === "sign_in_required";
  const readOnlySnapshot = getSharedNoteReadOnlySnapshot(
    snapshot,
    fallbackSnapshot,
  );
  const activeSnapshot =
    accessRevoked && readOnlySnapshot ? readOnlySnapshot : snapshot;
  const collaborationActive = !accessRevoked && !requiresSignIn;
  const readyNote = authorization.state === "ready" ? authorization.note : null;
  // hasCollaborationAccess is true here because the read surface re-checks
  // actual access from the comments query before enabling composition.
  const canComposeComments =
    collaborationActive &&
    readyNote !== null &&
    canComposeSharedNoteComments({
      capability: readyNote.capability,
      hasCollaborationAccess: true,
      manageAccess: readyNote.manageAccess,
    });

  if (
    shouldRenderSharedNoteUnavailable({
      accessRevoked,
      hasFallbackSnapshot: Boolean(readOnlySnapshot),
      revokedBehavior,
    })
  ) {
    return <SharedNoteUnavailable />;
  }

  const accessNotice = accessRevoked ? (
    <SharedNoteEditNotice>
      Your editing access changed. The view-only shared note is still available.
    </SharedNoteEditNotice>
  ) : null;
  const signInNotice = requiresSignIn ? (
    <SharedNoteEditNotice>
      Your session expired.{" "}
      <a className="underline underline-offset-4" href="/auth/?flow=web">
        Sign in again
      </a>{" "}
      to continue editing.
    </SharedNoteEditNotice>
  ) : null;

  const viewer = (
    <SharedNoteViewer
      accessLabel={
        accessRevoked
          ? (fallbackAccessLabel ?? "Shared note · View only")
          : requiresSignIn
            ? "Shared note · Sign in required"
            : accessLabel
      }
      actions={actions}
      documentContent={
        <SharedNoteReader
          canCompose={canComposeComments}
          manageAccess={readyNote?.manageAccess ?? false}
          resolveAttachment={resolveAttachment}
          shareId={activeSnapshot.shareId}
          signedIn={signedIn && collaborationActive}
          snapshot={activeSnapshot}
        />
      }
      notice={accessNotice ?? signInNotice}
      resolveAttachment={resolveAttachment}
      snapshot={activeSnapshot}
    />
  );

  return (
    <>
      {viewer}
      {chat?.(activeSnapshot)}
    </>
  );
}

function SharedNoteEditNotice({ children }: { children: React.ReactNode }) {
  return (
    <div className="border-color-subtle bg-surface-subtle mb-6 rounded-2xl border px-4 py-3 text-sm leading-6">
      {children}
    </div>
  );
}
