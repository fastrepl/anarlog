import { createFileRoute, redirect } from "@tanstack/react-router";
import { useCallback } from "react";

import { AccountSharedNoteActions } from "@/components/shared-note-actions";
import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import {
  SharedNoteLoading,
  SharedNoteUnavailable,
  SharedNoteViewer,
} from "@/components/shared-note-viewer";
import { fetchUser } from "@/functions/auth";
import {
  createAuthenticatedSharedAttachmentDownload,
  readAuthenticatedSharedNote,
} from "@/functions/shared-notes";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import {
  getPrivateShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import { shareIdSchema } from "@/lib/shared-notes";

export const Route = createFileRoute("/share/$shareId")({
  beforeLoad: async ({ location }) => {
    prepareShareRoutePrivacy();
    const user = await fetchUser();
    if (!user) {
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          redirect: location.pathname,
        },
      });
    }
    return { user };
  },
  loader: async ({ params }) => {
    const shareId = shareIdSchema.safeParse(params.shareId);
    if (!shareId.success) {
      return { note: null };
    }
    return {
      note: await readAuthenticatedSharedNote({ data: shareId.data }),
    };
  },
  head: getPrivateShareHead,
  headers: () => privateShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const { note } = Route.useLoaderData();
  const resolveAttachment = useCallback<SharedAttachmentResolver>(
    (attachment) =>
      createAuthenticatedSharedAttachmentDownload({
        data: {
          shareId: note?.snapshot.shareId ?? "",
          attachmentId: attachment.id,
        },
      }),
    [note?.snapshot.shareId],
  );
  if (!note) {
    return <SharedNoteUnavailable />;
  }

  return (
    <SharedNoteViewer
      snapshot={note.snapshot}
      resolveAttachment={resolveAttachment}
      accessLabel="Shared with you · View only"
      actions={<AccountSharedNoteActions shareId={note.snapshot.shareId} />}
    />
  );
}
