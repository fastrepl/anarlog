import { createFileRoute } from "@tanstack/react-router";
import { useCallback } from "react";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import {
  PublicSharedNoteActions,
} from "@/components/shared-note-actions";
import {
  SharedNoteLoading,
  SharedNoteTransientError,
  SharedNoteUnavailable,
  SharedNoteViewer,
} from "@/components/shared-note-viewer";
import { readPublicSharedNote } from "@/functions/shared-notes";
import {
  getPublicShareHead,
  publicShareHeaders,
} from "@/lib/shared-note-meta";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import { fetchPublicSharedAttachmentDownload } from "@/lib/shared-note-api";
import {
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
      return { result: { status: "unavailable" } as const };
    }
    return {
      result: await readPublicSharedNote({ data: publicSlug.data }),
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
  const { result } = Route.useLoaderData();
  const { publicSlug } = Route.useParams();
  const { scheme } = Route.useSearch();
  const resolveAttachment = useCallback<SharedAttachmentResolver>(
    (attachment, signal) =>
      fetchPublicSharedAttachmentDownload(
        publicSlug,
        attachment.id,
        signal,
      ),
    [publicSlug],
  );
  if (result.status === "error") {
    return <SharedNoteTransientError />;
  }
  if (result.status === "unavailable") {
    return <SharedNoteUnavailable />;
  }

  return (
    <SharedNoteViewer
      snapshot={result.snapshot}
      resolveAttachment={resolveAttachment}
      accessLabel="Public note · View only"
      actions={
        <PublicSharedNoteActions publicSlug={publicSlug} scheme={scheme} />
      }
    />
  );
}
