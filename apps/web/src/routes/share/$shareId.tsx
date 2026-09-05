import { createFileRoute, redirect } from "@tanstack/react-router";
import { useCallback } from "react";

import {
  AccountSharedNoteActions,
  StableSharedNoteActions,
} from "@/components/shared-note-actions";
import { SharedNoteChatPanel } from "@/components/shared-note-chat-panel";
import { SharedNoteCollaboration } from "@/components/shared-note-collaboration";
import { SharedNoteCollaborationViewer } from "@/components/shared-note-collaboration-viewer";
import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import {
  SharedNoteLoading,
  SharedNoteTransientError,
  SharedNoteUnavailable,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import {
  createAuthenticatedSharedAttachmentDownload,
  readAuthenticatedSharedNote,
} from "@/functions/shared-notes";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import {
  fetchStableSharedAttachmentDownload,
  fetchStableSharedNotePreviewResult,
  fetchStableSharedNoteResult,
} from "@/lib/shared-note-api";
import {
  formatAuthenticatedSharedNoteAccessLabel,
  shouldUseAuthenticatedSharedNoteAccessLabel,
} from "@/lib/shared-note-collaboration";
import {
  getPrivateShareHead,
  privateShareHeaders,
  publicShareHeaders,
  getStableShareHead,
} from "@/lib/shared-note-meta";
import {
  buildSharedNoteWebPath,
  sharedNoteDesktopSchemeSchema,
  shareIdSchema,
} from "@/lib/shared-notes";

export const Route = createFileRoute("/share/$shareId")({
  validateSearch: (search) => ({
    scheme: sharedNoteDesktopSchemeSchema.parse(search.scheme),
  }),
  beforeLoad: async () => {
    prepareShareRoutePrivacy();
    return { user: await fetchUser() };
  },
  loaderDeps: ({ search }) => ({ scheme: search.scheme }),
  loader: async ({ context, deps, location, params }) => {
    const shareId = shareIdSchema.safeParse(params.shareId);
    if (!shareId.success) {
      return {
        authenticatedResult: null,
        preview: null,
        stableResult: { status: "unavailable" } as const,
      };
    }
    const [authenticatedResult, previewResult, stableResult] =
      await Promise.all([
        context.user
          ? readAuthenticatedSharedNote({ data: shareId.data })
          : null,
        fetchStableSharedNotePreviewResult(shareId.data),
        fetchStableSharedNoteResult(shareId.data),
      ]);
    if (!context.user && stableResult.status === "unavailable") {
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          redirect: buildSharedNoteWebPath(location.pathname, deps.scheme),
        },
      });
    }
    return {
      authenticatedResult,
      preview: previewResult.status === "ready" ? previewResult.preview : null,
      stableResult,
    };
  },
  head: ({ loaderData, params }) =>
    loaderData?.stableResult.status === "ready"
      ? getStableShareHead(
          params.shareId,
          loaderData.stableResult.accessScope,
          loaderData.stableResult.snapshot,
          loaderData.preview,
        )
      : getPrivateShareHead(),
  headers: ({ loaderData }) =>
    loaderData?.stableResult.status === "ready" &&
    loaderData.stableResult.accessScope === "public"
      ? publicShareHeaders
      : privateShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const { authenticatedResult, preview, stableResult } = Route.useLoaderData();
  const { user } = Route.useRouteContext();
  const { scheme } = Route.useSearch();
  const authenticatedNote =
    authenticatedResult?.status === "ready" ? authenticatedResult.note : null;
  const stableNote = stableResult.status === "ready" ? stableResult : null;
  const snapshot = authenticatedNote?.snapshot ?? stableNote?.snapshot ?? null;
  const resolveAttachment = useCallback<SharedAttachmentResolver>(
    async (attachment, signal) => {
      if (authenticatedNote) {
        const download = await createAuthenticatedSharedAttachmentDownload({
          data: {
            shareId: authenticatedNote.snapshot.shareId,
            attachmentId: attachment.id,
          },
        });
        if (download) return download;
      }
      return stableNote
        ? fetchStableSharedAttachmentDownload(
            stableNote.snapshot.shareId,
            attachment.id,
            signal,
          )
        : null;
    },
    [authenticatedNote, stableNote],
  );
  if (
    !snapshot &&
    (authenticatedResult?.status === "error" || stableResult.status === "error")
  ) {
    return <SharedNoteTransientError />;
  }
  if (!snapshot) {
    return <SharedNoteUnavailable />;
  }

  const returnPath = buildSharedNoteWebPath(
    `/share/${encodeURIComponent(snapshot.shareId)}/`,
    scheme,
  );
  const stableAccessLabel =
    stableNote?.accessScope === "public"
      ? "Public note · View only"
      : stableNote?.accessScope === "link"
        ? "Anyone with the link · View only"
        : "Shared note · View only";
  const accessLabel =
    authenticatedNote &&
    (!stableNote ||
      shouldUseAuthenticatedSharedNoteAccessLabel(authenticatedNote))
      ? formatAuthenticatedSharedNoteAccessLabel(authenticatedNote)
      : stableAccessLabel;

  return (
    <>
      <SharedNoteCollaborationViewer
        key={snapshot.shareId}
        snapshot={snapshot}
        authenticatedNote={authenticatedNote}
        meetingMetadata={preview}
        resolveAttachment={resolveAttachment}
        signedIn={user !== null}
        accessLabel={accessLabel}
        collaboration={
          <SharedNoteCollaboration
            capability={authenticatedNote?.capability ?? "viewer"}
            currentUserId={user?.id ?? null}
            manageAccess={authenticatedNote?.manageAccess ?? false}
            returnPath={returnPath}
            shareId={snapshot.shareId}
          />
        }
        actions={
          authenticatedNote ? (
            <AccountSharedNoteActions
              canEdit={authenticatedNote.capability === "editor"}
              scheme={scheme}
              shareId={snapshot.shareId}
            />
          ) : (
            <StableSharedNoteActions
              canEdit={false}
              scheme={scheme}
              shareId={snapshot.shareId}
            />
          )
        }
        chat={(liveSnapshot) => (
          <SharedNoteChatPanel
            returnPath={returnPath}
            signedIn={user !== null}
            snapshot={liveSnapshot}
          />
        )}
      />
    </>
  );
}
