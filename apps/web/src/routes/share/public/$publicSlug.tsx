import { createFileRoute } from "@tanstack/react-router";
import { useCallback } from "react";

import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import {
  PublicSharedNoteActions,
} from "@/components/shared-note-actions";
import {
  SharedNoteLoading,
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
import { publicShareSlugSchema } from "@/lib/shared-notes";

export const Route = createFileRoute("/share/public/$publicSlug")({
  beforeLoad: () => prepareShareRoutePrivacy(),
  loader: async ({ params }) => {
    const publicSlug = publicShareSlugSchema.safeParse(params.publicSlug);
    if (!publicSlug.success) {
      return { snapshot: null };
    }
    return {
      snapshot: await readPublicSharedNote({ data: publicSlug.data }),
    };
  },
  head: ({ loaderData, params }) =>
    getPublicShareHead(params.publicSlug, loaderData?.snapshot),
  headers: () => publicShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const { snapshot } = Route.useLoaderData();
  const { publicSlug } = Route.useParams();
  const resolveAttachment = useCallback<SharedAttachmentResolver>(
    (attachment, signal) =>
      fetchPublicSharedAttachmentDownload(
        publicSlug,
        attachment.id,
        signal,
      ),
    [publicSlug],
  );
  if (!snapshot) {
    return <SharedNoteUnavailable />;
  }

  return (
    <SharedNoteViewer
      snapshot={snapshot}
      resolveAttachment={resolveAttachment}
      accessLabel="Public note · View only"
      actions={<PublicSharedNoteActions publicSlug={publicSlug} />}
    />
  );
}
