import { lazy, Suspense, useState } from "react";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import { SharedNoteReader } from "@/components/shared-note-reader";
import {
  sharedSecondaryButtonClassName,
  SharedNoteUnavailable,
  SharedNoteViewer,
} from "@/components/shared-note-viewer";
import { canComposeSharedNoteComments } from "@/lib/shared-note-collaboration";
import {
  canEditSharedNoteOnWeb,
  getSharedNoteReadOnlySnapshot,
  getSharedNoteWebEditPreparationMessage,
  hasUnsupportedSharedNoteEditorNode,
  resolveSharedNoteViewerAuthorization,
  shouldRenderSharedNoteUnavailable,
  syncSharedNoteViewerAuthorization,
  type SharedNoteViewerAuthorization,
} from "@/lib/shared-note-editing";
import type {
  AuthenticatedSharedNote,
  SharedNoteSnapshot,
  SharedNoteWebEditSnapshot,
} from "@/lib/shared-notes";

const SharedNoteEditorSurface = lazy(() =>
  import("@/components/shared-note-editor-surface").then((module) => ({
    default: module.SharedNoteEditorSurface,
  })),
);

export function SharedNoteEditableViewer({
  accessLabel,
  actions,
  authenticatedNote,
  collaboration,
  fallbackAccessLabel,
  fallbackSnapshot,
  onAccessChanged,
  resolveAttachment,
  revokedBehavior,
  signedIn,
  snapshot: initialSnapshot,
}: {
  accessLabel: string;
  actions?: React.ReactNode;
  authenticatedNote: AuthenticatedSharedNote | null;
  collaboration?: React.ReactNode;
  fallbackAccessLabel?: string;
  fallbackSnapshot?: SharedNoteSnapshot | null;
  onAccessChanged?: () => Promise<AuthenticatedSharedNote | null>;
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
  const [isEditing, setIsEditing] = useState(false);

  if (
    !isEditing &&
    (source.snapshot !== initialSnapshot ||
      source.authenticatedNote !== authenticatedNote)
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
  const hasUnsupportedContent = hasUnsupportedSharedNoteEditorNode(
    activeSnapshot.body,
  );
  const canEdit =
    authorization.state === "ready" &&
    canEditSharedNoteOnWeb(authorization.note) &&
    !hasUnsupportedContent;
  const preparationMessage = getSharedNoteWebEditPreparationMessage(
    authorization.state === "ready" ? authorization.note : null,
    hasUnsupportedContent,
  );
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

  const preparationNotice = preparationMessage ? (
    <SharedNoteEditNotice>{preparationMessage}</SharedNoteEditNotice>
  ) : null;
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

  return (
    <SharedNoteViewer
      accessLabel={
        accessRevoked
          ? (fallbackAccessLabel ?? "Shared note · View only")
          : requiresSignIn
            ? "Shared note · Sign in required"
            : accessLabel
      }
      actions={actions}
      collaboration={
        accessRevoked || requiresSignIn ? undefined : collaboration
      }
      documentContent={
        isEditing ? (
          <Suspense
            fallback={
              <p className="text-color-muted py-10 text-center text-sm">
                Loading editor…
              </p>
            }
          >
            <SharedNoteEditorSurface
              key={`${activeSnapshot.shareId}:${activeSnapshot.contentRevision}`}
              snapshot={activeSnapshot}
              onCancel={() => setIsEditing(false)}
              onReloadLatest={(edited) => {
                applyEditedSnapshot(edited);
                if (
                  !edited.webEditable ||
                  hasUnsupportedSharedNoteEditorNode(edited.snapshot.body)
                ) {
                  setIsEditing(false);
                }
              }}
              onSaved={(edited) => {
                applyEditedSnapshot(edited);
                setIsEditing(false);
              }}
              onUnavailable={(reason) => {
                if (reason === "access_changed") {
                  setAuthorization((current) => ({
                    ...current,
                    state: "access_changed",
                  }));
                  const refresh = onAccessChanged?.();
                  if (refresh) {
                    void refresh
                      .then((note) => {
                        setAuthorization((current) =>
                          current.state === "sign_in_required"
                            ? current
                            : resolveSharedNoteViewerAuthorization(note),
                        );
                      })
                      .catch(() => {});
                  }
                } else {
                  setAuthorization({
                    note: null,
                    state: "sign_in_required",
                  });
                }
                setIsEditing(false);
              }}
            />
          </Suspense>
        ) : (
          <SharedNoteReader
            canCompose={canComposeComments}
            manageAccess={readyNote?.manageAccess ?? false}
            resolveAttachment={resolveAttachment}
            shareId={activeSnapshot.shareId}
            signedIn={signedIn && collaborationActive}
            snapshot={activeSnapshot}
          />
        )
      }
      headerActions={
        canEdit && !isEditing ? (
          <button
            type="button"
            className={sharedSecondaryButtonClassName}
            onClick={() => setIsEditing(true)}
          >
            Edit
          </button>
        ) : undefined
      }
      notice={accessNotice ?? signInNotice ?? preparationNotice}
      resolveAttachment={resolveAttachment}
      showTitle={!isEditing}
      snapshot={activeSnapshot}
    />
  );

  function applyEditedSnapshot(edited: SharedNoteWebEditSnapshot) {
    setSource({ authenticatedNote, snapshot: initialSnapshot });
    setSnapshot(edited.snapshot);
    setAuthorization((current) => ({
      state: "ready",
      note: current.note
        ? {
            ...current.note,
            accessVersion: edited.accessVersion,
            snapshot: edited.snapshot,
            webEditable: edited.webEditable,
          }
        : null,
    }));
  }
}

function SharedNoteEditNotice({ children }: { children: React.ReactNode }) {
  return (
    <div className="border-color-subtle bg-surface-subtle mb-6 rounded-2xl border px-4 py-3 text-sm leading-6">
      {children}
    </div>
  );
}
