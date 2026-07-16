import { createFileRoute } from "@tanstack/react-router";

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
  if (!snapshot) {
    return <SharedNoteUnavailable />;
  }

  return (
    <SharedNoteViewer
      snapshot={snapshot}
      accessLabel="Public note · View only"
      actions={<PublicSharedNoteActions publicSlug={publicSlug} />}
    />
  );
}
