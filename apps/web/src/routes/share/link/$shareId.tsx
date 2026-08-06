import { useQuery } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";
import { useCallback } from "react";

import { useShareRouteContinuation } from "@/components/share-route-continuation";
import { LinkSharedNoteActions } from "@/components/shared-note-actions";
import { SharedNoteChatPanel } from "@/components/shared-note-chat-panel";
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
} from "@/functions/shared-notes";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import {
  fetchLinkSharedAttachmentDownload,
  fetchLinkSharedNotePreviewResult,
  fetchLinkSharedNoteResult,
} from "@/lib/shared-note-api";
import {
  formatAuthenticatedSharedNoteAccessLabel,
  shouldUseAuthenticatedSharedNoteAccessLabel,
} from "@/lib/shared-note-collaboration";
import {
  getLinkShareHead,
  getPrivateShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import {
  getLinkSharedNoteFallbackSnapshot,
  getLinkSharedNoteRouteGate,
} from "@/lib/shared-note-route-state";
import {
  buildSharedNoteWebPath,
  linkSharePreviewTokenSchema,
  sharedNoteDesktopSchemeSchema,
  shareIdSchema,
  type SharedNotePreview,
} from "@/lib/shared-notes";

export const Route = createFileRoute("/share/link/$shareId")({
  validateSearch: (search) => ({
    scheme: sharedNoteDesktopSchemeSchema.parse(search.scheme),
    preview: linkSharePreviewTokenSchema
      .optional()
      .catch(undefined)
      .parse(search.preview),
  }),
  beforeLoad: async () => {
    prepareShareRoutePrivacy();
    return { user: await fetchUser() };
  },
  loaderDeps: ({ search }) => ({ previewToken: search.preview }),
  loader: async ({ deps, params }) => {
    const shareId = shareIdSchema.safeParse(params.shareId);
    if (!shareId.success || !deps.previewToken) {
      return { preview: null, previewToken: undefined };
    }
    const result = await fetchLinkSharedNotePreviewResult(
      shareId.data,
      deps.previewToken,
    );
    return {
      preview: result.status === "ready" ? result.preview : null,
      previewToken: deps.previewToken,
    };
  },
  head: ({ loaderData, params }) =>
    loaderData
      ? getLinkShareHead(
          params.shareId,
          loaderData.previewToken,
          loaderData.preview,
        )
      : getPrivateShareHead(),
  headers: () => privateShareHeaders,
  component: Component,
});

function Component() {
  const { preview } = Route.useLoaderData();
  const { shareId } = Route.useParams();
  const { user } = Route.useRouteContext();
  return (
    <ClientOnly fallback={<SharedNoteLoading />}>
      <LinkSharedNoteClient
        currentUserId={user?.id ?? null}
        meetingMetadata={preview}
        shareId={shareId}
      />
    </ClientOnly>
  );
}

function LinkSharedNoteClient({
  currentUserId,
  meetingMetadata,
  shareId,
}: {
  currentUserId: string | null;
  meetingMetadata: SharedNotePreview | null;
  shareId: string;
}) {
  const { scheme } = Route.useSearch();
  const pathname = window.location.pathname;
  const continuation = useShareRouteContinuation(pathname);
  const hasToken = continuation.token !== null;
  const validShareId = shareIdSchema.safeParse(shareId);
  const snapshotQuery = useQuery({
    queryKey: ["shared-note-link", shareId],
    queryFn: ({ signal }) => {
      return continuation.token && validShareId.success
        ? fetchLinkSharedNoteResult(
            validShareId.data,
            continuation.token,
            signal,
          )
        : Promise.resolve({ status: "unavailable" } as const);
    },
    enabled: hasToken && validShareId.success,
    gcTime: 0,
    retry: false,
    staleTime: 0,
  });
  const authenticatedQuery = useQuery({
    queryKey: ["shared-note-authenticated", shareId, currentUserId],
    queryFn: () => readAuthenticatedSharedNote({ data: shareId }),
    enabled: currentUserId !== null && validShareId.success,
    retry: false,
    staleTime: 0,
  });
  const authenticatedNote =
    authenticatedQuery.data?.status === "ready"
      ? authenticatedQuery.data.note
      : null;
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
      return continuation.token
        ? fetchLinkSharedAttachmentDownload(
            shareId,
            continuation.token,
            attachment.id,
            signal,
          )
        : Promise.resolve(null);
    },
    [authenticatedNote, continuation.token, shareId],
  );

  if (!validShareId.success) {
    return <SharedNoteUnavailable />;
  }
  const routeGate = getLinkSharedNoteRouteGate({
    authenticatedNotePending:
      currentUserId !== null && authenticatedQuery.isPending,
    continuationFailed: continuation.isError,
    continuationPending: continuation.isPending,
    hasAuthenticatedNote: authenticatedNote !== null,
    linkSnapshotPending: hasToken && snapshotQuery.isPending,
  });
  if (routeGate === "loading") {
    return <SharedNoteLoading />;
  }
  if (routeGate === "continuation-error") {
    return (
      <SharedNoteTransientError
        retry={() => {
          void continuation.retry();
          if (currentUserId !== null) void authenticatedQuery.refetch();
        }}
      />
    );
  }
  if (
    !authenticatedNote &&
    (snapshotQuery.isError ||
      snapshotQuery.data?.status === "error" ||
      (authenticatedQuery.isError && !hasToken))
  ) {
    return (
      <SharedNoteTransientError
        retry={() => {
          void authenticatedQuery.refetch();
          if (hasToken) void snapshotQuery.refetch();
        }}
      />
    );
  }
  const linkSnapshot =
    snapshotQuery.data?.status === "ready" ? snapshotQuery.data.snapshot : null;
  const snapshot = authenticatedNote?.snapshot ?? linkSnapshot;
  if (!snapshot) {
    return <SharedNoteUnavailable />;
  }
  const fallbackSnapshot = getLinkSharedNoteFallbackSnapshot({
    authenticatedSnapshot: authenticatedNote?.snapshot ?? null,
    linkSnapshot,
  });
  const returnPath = buildSharedNoteWebPath(pathname, scheme);

  return (
    <>
      <SharedNoteEditableViewer
        key={snapshot.shareId}
        snapshot={snapshot}
        authenticatedNote={authenticatedNote}
        fallbackAccessLabel={
          linkSnapshot
            ? "Anyone with the link · View only"
            : "Shared note · View only"
        }
        fallbackSnapshot={fallbackSnapshot}
        meetingMetadata={meetingMetadata}
        resolveAttachment={resolveAttachment}
        revokedBehavior="read-only"
        signedIn={currentUserId !== null}
        accessLabel={
          authenticatedNote &&
          shouldUseAuthenticatedSharedNoteAccessLabel(authenticatedNote)
            ? formatAuthenticatedSharedNoteAccessLabel(authenticatedNote)
            : "Anyone with the link · View only"
        }
        actions={
          <LinkSharedNoteActions
            canEdit={authenticatedNote?.capability === "editor"}
            pathname={pathname}
            scheme={scheme}
            shareId={validShareId.data}
          />
        }
        chat={(liveSnapshot) => (
          <SharedNoteChatPanel
            returnPath={returnPath}
            signedIn={currentUserId !== null}
            snapshot={liveSnapshot}
          />
        )}
      />
    </>
  );
}
