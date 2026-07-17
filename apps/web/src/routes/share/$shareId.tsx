import { createFileRoute, redirect } from "@tanstack/react-router";
import { useCallback } from "react";

import { AccountSharedNoteActions } from "@/components/shared-note-actions";
import { SharedNoteCollaboration } from "@/components/shared-note-collaboration";
import type { SharedAttachmentResolver } from "@/components/shared-note-document";
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
} from "@/functions/shared-notes";
import { prepareShareRoutePrivacy } from "@/lib/share-route-privacy";
import { formatAuthenticatedSharedNoteAccessLabel } from "@/lib/shared-note-collaboration";
import {
  getPrivateShareHead,
  privateShareHeaders,
} from "@/lib/shared-note-meta";
import {
  buildSharedNoteWebPath,
  sharedNoteDesktopSchemeSchema,
  shareIdSchema,
} from "@/lib/shared-notes";

export const Route = createFileRoute("/share/$shareId")({
  validateSearch: (search) => ({
    scheme: sharedNoteDesktopSchemeSchema.parse(search.scheme),
  }),
  beforeLoad: async ({ location, search }) => {
    prepareShareRoutePrivacy();
    const user = await fetchUser();
    if (!user) {
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          redirect: buildSharedNoteWebPath(location.pathname, search.scheme),
        },
      });
    }
    return { user };
  },
  loader: async ({ params }) => {
    const shareId = shareIdSchema.safeParse(params.shareId);
    if (!shareId.success) {
      return { result: { status: "unavailable" } as const };
    }
    return {
      result: await readAuthenticatedSharedNote({ data: shareId.data }),
    };
  },
  head: getPrivateShareHead,
  headers: () => privateShareHeaders,
  pendingComponent: SharedNoteLoading,
  component: Component,
});

function Component() {
  const { result } = Route.useLoaderData();
  const { user } = Route.useRouteContext();
  const { scheme } = Route.useSearch();
  const note = result.status === "ready" ? result.note : null;
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
  if (result.status === "error") {
    return <SharedNoteTransientError />;
  }
  if (result.status === "unavailable" || !note) {
    return <SharedNoteUnavailable />;
  }

  return (
    <SharedNoteViewer
      snapshot={note.snapshot}
      resolveAttachment={resolveAttachment}
      accessLabel={formatAuthenticatedSharedNoteAccessLabel(note)}
      collaboration={
        <SharedNoteCollaboration
          capability={note.capability}
          currentUserId={user.id}
          manageAccess={note.manageAccess}
          returnPath={buildSharedNoteWebPath(
            `/share/${encodeURIComponent(note.snapshot.shareId)}/`,
            scheme,
          )}
          shareId={note.snapshot.shareId}
        />
      }
      actions={
        <AccountSharedNoteActions
          scheme={scheme}
          shareId={note.snapshot.shareId}
        />
      }
    />
  );
}
