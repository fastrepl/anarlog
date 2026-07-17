import { createFileRoute } from "@tanstack/react-router";
import { useCallback } from "react";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import { PublicSharedNoteActions } from "@/components/shared-note-actions";
import { SharedNoteCollaboration } from "@/components/shared-note-collaboration";
import {
  SharedNoteLoading,
  SharedNoteTransientError,
  SharedNoteUnavailable,
  SharedNoteViewer,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import {
  createAuthenticatedSharedAttachmentDownload,
  readAuthenticatedSharedNote,
  readPublicSharedNote,
} from "@/functions/shared-notes";
import {
  formatAuthenticatedSharedNoteAccessLabel,
  shouldUseAuthenticatedSharedNoteAccessLabel,
} from "@/lib/shared-note-collaboration";
import {
  getPublicShareHead,
  publicShareHeaders,
} from "@/lib/shared-note-meta";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import { fetchPublicSharedAttachmentDownload } from "@/lib/shared-note-api";
import {
  buildSharedNoteWebPath,
  publicShareSlugSchema,
  sharedNoteDesktopSchemeSchema,
} from "@/lib/shared-notes";

export const Route = createFileRoute("/share/public/$publicSlug")({
  validateSearch: (search) => ({
    scheme: sharedNoteDesktopSchemeSchema.parse(search.scheme),
  }),
  beforeLoad: () => prepareShareRoutePrivacy(),
  loader: async ({ params }) => {
    const publicSlug = publicShareSlugSchema.safeParse(params.publicSlug);
    if (!publicSlug.success) {
      return {
        authenticatedResult: null,
        result: { status: "unavailable" } as const,
        user: null,
      };
    }
    const [result, user] = await Promise.all([
      readPublicSharedNote({ data: publicSlug.data }),
      fetchUser(),
    ]);
    const authenticatedResult =
      user && result.status === "ready"
        ? await readAuthenticatedSharedNote({ data: result.snapshot.shareId })
        : null;
    return {
      authenticatedResult,
      result,
      user,
    };
  },
  head: ({ loaderData, params }) =>
    getPublicShareHead(
      params.publicSlug,
      loaderData?.result.status === "ready"
        ? loaderData.result.snapshot
        : null,
    ),
  headers: () => publicShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const { authenticatedResult, result, user } = Route.useLoaderData();
  const { publicSlug } = Route.useParams();
  const { scheme } = Route.useSearch();
  const authenticatedNote =
    authenticatedResult?.status === "ready"
      ? authenticatedResult.note
      : null;
  const resolveAttachment = useCallback<SharedAttachmentResolver>(
    (attachment, signal) =>
      authenticatedNote
        ? createAuthenticatedSharedAttachmentDownload({
            data: {
              shareId: authenticatedNote.snapshot.shareId,
              attachmentId: attachment.id,
            },
          })
        : fetchPublicSharedAttachmentDownload(
            publicSlug,
            attachment.id,
            signal,
          ),
    [authenticatedNote, publicSlug],
  );
  if (result.status === "error") {
    return <SharedNoteTransientError />;
  }
  if (result.status === "unavailable") {
    return <SharedNoteUnavailable />;
  }

  const snapshot = authenticatedNote?.snapshot ?? result.snapshot;
  const accessLabel =
    authenticatedNote &&
    shouldUseAuthenticatedSharedNoteAccessLabel(authenticatedNote)
      ? formatAuthenticatedSharedNoteAccessLabel(authenticatedNote)
      : "Public note · View only";

  return (
    <SharedNoteViewer
      snapshot={snapshot}
      resolveAttachment={resolveAttachment}
      accessLabel={accessLabel}
      collaboration={
        <SharedNoteCollaboration
          capability={authenticatedNote?.capability ?? "viewer"}
          currentUserId={user?.id ?? null}
          manageAccess={authenticatedNote?.manageAccess ?? false}
          returnPath={buildSharedNoteWebPath(
            `/share/public/${encodeURIComponent(publicSlug)}/`,
            scheme,
          )}
          shareId={snapshot.shareId}
        />
      }
      actions={
        <PublicSharedNoteActions publicSlug={publicSlug} scheme={scheme} />
      }
    />
  );
}
