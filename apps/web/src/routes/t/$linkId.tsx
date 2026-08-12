import { useQuery } from "@tanstack/react-query";
import { ClientOnly, createFileRoute } from "@tanstack/react-router";

import {
  SharedNoteLoading,
  SharedNoteTransientError,
  SharedNoteUnavailable,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import { fetchShortLinkSharedNotePreviewResult } from "@/lib/shared-note-api";
import {
  getPrivateShareHead,
  getShortLinkShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import {
  shareLinkIdSchema,
  sharedNoteDesktopSchemeSchema,
  type SharedNoteDesktopScheme,
} from "@/lib/shared-notes";
import { LinkSharedNoteClient } from "@/routes/share/link/$shareId";

export const Route = createFileRoute("/t/$linkId")({
  validateSearch: (search) => ({
    scheme: sharedNoteDesktopSchemeSchema.parse(search.scheme),
  }),
  beforeLoad: async () => {
    prepareShareRoutePrivacy();
    return { user: await fetchUser() };
  },
  loader: async ({ params }) => {
    const linkId = shareLinkIdSchema.safeParse(params.linkId);
    if (!linkId.success) return { status: "unavailable" } as const;
    return fetchShortLinkSharedNotePreviewResult(linkId.data);
  },
  head: ({ loaderData, params }) =>
    loaderData?.status === "ready"
      ? getShortLinkShareHead(params.linkId, loaderData.preview)
      : getPrivateShareHead(),
  headers: () => privateShareHeaders,
  component: Component,
});

function Component() {
  const previewResult = Route.useLoaderData();
  const { linkId } = Route.useParams();
  const { scheme } = Route.useSearch();
  const { user } = Route.useRouteContext();
  if (previewResult.status === "unavailable") {
    return <SharedNoteUnavailable />;
  }

  return (
    <ClientOnly fallback={<SharedNoteLoading />}>
      {previewResult.status === "ready" ? (
        <LinkSharedNoteClient
          currentUserId={user?.id ?? null}
          meetingMetadata={previewResult.preview}
          scheme={scheme}
          shareId={previewResult.preview.shareId}
        />
      ) : (
        <ShortLinkSharedNoteClient
          currentUserId={user?.id ?? null}
          linkId={linkId}
          scheme={scheme}
        />
      )}
    </ClientOnly>
  );
}

function ShortLinkSharedNoteClient({
  currentUserId,
  linkId,
  scheme,
}: {
  currentUserId: string | null;
  linkId: string;
  scheme: SharedNoteDesktopScheme;
}) {
  const previewQuery = useQuery({
    queryKey: ["shared-note-short-link-preview", linkId],
    queryFn: ({ signal }) =>
      fetchShortLinkSharedNotePreviewResult(linkId, signal),
    gcTime: 0,
    retry: false,
    staleTime: 0,
  });

  if (previewQuery.isPending) return <SharedNoteLoading />;
  if (previewQuery.isError || previewQuery.data.status === "error") {
    return (
      <SharedNoteTransientError
        retry={() => {
          void previewQuery.refetch();
        }}
      />
    );
  }
  if (previewQuery.data.status === "unavailable") {
    return <SharedNoteUnavailable />;
  }

  return (
    <LinkSharedNoteClient
      currentUserId={currentUserId}
      meetingMetadata={previewQuery.data.preview}
      scheme={scheme}
      shareId={previewQuery.data.preview.shareId}
    />
  );
}
