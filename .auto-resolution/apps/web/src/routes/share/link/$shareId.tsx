import { useQuery } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";

import { LinkSharedNoteActions } from "@/components/shared-note-actions";
import {
  SharedNoteLoading,
  SharedNoteUnavailable,
  SharedNoteViewer,
} from "@/components/shared-note-viewer";
import {
  getShareRouteToken,
  prepareShareRoutePrivacy,
} from "@/lib/share-route-privacy";
import { fetchLinkSharedNote } from "@/lib/shared-note-api";
import {
  getPrivateShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import { shareIdSchema } from "@/lib/shared-notes";

export const Route = createFileRoute("/share/link/$shareId")({
  beforeLoad: () => prepareShareRoutePrivacy(),
  head: getPrivateShareHead,
  headers: () => privateShareHeaders,
  component: Component,
});

function Component() {
  const { shareId } = Route.useParams();
  return (
    <ClientOnly fallback={<SharedNoteLoading />}>
      <LinkSharedNoteClient shareId={shareId} />
    </ClientOnly>
  );
}

function LinkSharedNoteClient({ shareId }: { shareId: string }) {
  const pathname = window.location.pathname;
  const hasToken = Boolean(getShareRouteToken(pathname));
  const validShareId = shareIdSchema.safeParse(shareId);
  const snapshotQuery = useQuery({
    queryKey: ["shared-note-link", shareId],
    queryFn: ({ signal }) => {
      const token = getShareRouteToken(pathname);
      return token && validShareId.success
        ? fetchLinkSharedNote(validShareId.data, token, signal)
        : Promise.resolve(null);
    },
    enabled: hasToken && validShareId.success,
    gcTime: 0,
    retry: false,
    staleTime: 0,
  });

  if (!hasToken || !validShareId.success) {
    return <SharedNoteUnavailable />;
  }
  if (snapshotQuery.isPending) {
    return <SharedNoteLoading />;
  }
  if (!snapshotQuery.data) {
    return <SharedNoteUnavailable />;
  }

  return (
    <SharedNoteViewer
      snapshot={snapshotQuery.data}
      accessLabel="Anyone with the link · View only"
      actions={
        <LinkSharedNoteActions
          pathname={pathname}
          shareId={validShareId.data}
        />
      }
    />
  );
}
