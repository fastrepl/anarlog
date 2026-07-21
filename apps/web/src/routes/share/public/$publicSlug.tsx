import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useCallback } from "react";

import { PublicSharedNoteActions } from "@/components/shared-note-actions";
import { SharedNoteChatPanel } from "@/components/shared-note-chat-panel";
import { SharedNoteCollaboration } from "@/components/shared-note-collaboration";
import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import { SharedNoteEditableViewer } from "@/components/shared-note-editable-viewer";
import {
  SharedNoteLoading,
  SharedNoteTransientError,
  SharedNoteUnavailable,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import {
  createAuthenticatedSharedAttachmentDownload,
  readAuthenticatedSharedNote,
  readPublicSharedNote,
} from "@/functions/shared-notes";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import { fetchPublicSharedAttachmentDownload } from "@/lib/shared-note-api";
import {
  formatAuthenticatedSharedNoteAccessLabel,
  shouldUseAuthenticatedSharedNoteAccessLabel,
} from "@/lib/shared-note-collaboration";
import { getPublicShareHead, publicShareHeaders } from "@/lib/shared-note-meta";
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
      loaderData?.result.status === "ready" ? loaderData.result.snapshot : null,
    ),
  headers: () => publicShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const router = useRouter();
  const { authenticatedResult, result, user } = Route.useLoaderData();
  const { publicSlug } = Route.useParams();
  const { scheme } = Route.useSearch();
  const authenticatedNote =
    authenticatedResult?.status === "ready" ? authenticatedResult.note : null;
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
      return fetchPublicSharedAttachmentDownload(
        publicSlug,
        attachment.id,
        signal,
      );
    },
    [authenticatedNote, publicSlug],
  );
  if (result.status === "error") {
    return <SharedNoteTransientError />;
  }
  if (result.status === "unavailable") {
    return <SharedNoteUnavailable />;
  }

  const snapshot = authenticatedNote?.snapshot ?? result.snapshot;
  const returnPath = buildSharedNoteWebPath(
    `/share/public/${encodeURIComponent(publicSlug)}/`,
    scheme,
  );
  const accessLabel =
    authenticatedNote &&
    shouldUseAuthenticatedSharedNoteAccessLabel(authenticatedNote)
      ? formatAuthenticatedSharedNoteAccessLabel(authenticatedNote)
      : "Public note · View only";

  return (
    <>
      <SharedNoteEditableViewer
        key={snapshot.shareId}
      snapshot={snapshot}
      authenticatedNote={authenticatedNote}
      fallbackAccessLabel="Public note · View only"
      fallbackSnapshot={result.snapshot}
      onAccessChanged={async () => {
        await router.invalidate();
        const refreshed = await readAuthenticatedSharedNote({
          data: snapshot.shareId,
        });
        return refreshed.status === "ready" ? refreshed.note : null;
      }}
      resolveAttachment={resolveAttachment}
      revokedBehavior="read-only"
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
        <PublicSharedNoteActions publicSlug={publicSlug} scheme={scheme} />
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
