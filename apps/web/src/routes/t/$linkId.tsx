import { ClientOnly, createFileRoute } from "@tanstack/react-router";

import {
  SharedNoteLoading,
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
    if (!linkId.success) return null;
    const result = await fetchShortLinkSharedNotePreviewResult(linkId.data);
    return result.status === "ready" ? result.preview : null;
  },
  head: ({ loaderData, params }) =>
    loaderData
      ? getShortLinkShareHead(params.linkId, loaderData)
      : getPrivateShareHead(),
  headers: () => privateShareHeaders,
  component: Component,
});

function Component() {
  const preview = Route.useLoaderData();
  const { scheme } = Route.useSearch();
  const { user } = Route.useRouteContext();
  if (!preview) return <SharedNoteUnavailable />;

  return (
    <ClientOnly fallback={<SharedNoteLoading />}>
      <LinkSharedNoteClient
        currentUserId={user?.id ?? null}
        meetingMetadata={preview}
        scheme={scheme}
        shareId={preview.shareId}
      />
    </ClientOnly>
  );
}
