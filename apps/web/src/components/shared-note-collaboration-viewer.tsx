import {
  collectSharedNoteComments,
  useSharedNoteComments,
} from "@/components/shared-note-comments-data";
import { SharedNoteCommentsDrawer } from "@/components/shared-note-comments-drawer";
import type { SharedAttachmentResolver } from "@/components/shared-note-document";
import { SharedNoteReader } from "@/components/shared-note-reader";
import { SharedNoteViewer } from "@/components/shared-note-viewer";
import {
  canComposeSharedNoteComments,
  hasSharedNoteCollaborationAccess,
} from "@/lib/shared-note-collaboration";
import { findFeaturedSharedNoteAudio } from "@/lib/shared-note-presentation";
import type {
  AuthenticatedSharedNote,
  SharedNoteSnapshot,
} from "@/lib/shared-notes";

export function SharedNoteCollaborationViewer({
  accessLabel,
  actions,
  authenticatedNote,
  chat,
  collaboration,
  meetingMetadata,
  resolveAttachment,
  signedIn,
  snapshot,
}: {
  accessLabel: string;
  actions?: React.ReactNode;
  authenticatedNote: AuthenticatedSharedNote | null;
  chat?: (snapshot: SharedNoteSnapshot) => React.ReactNode;
  collaboration?: React.ReactNode;
  meetingMetadata?: {
    meetingAt: string;
    participants: string[];
  } | null;
  resolveAttachment?: SharedAttachmentResolver;
  signedIn: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const commentsQuery = useSharedNoteComments({
    enabled: signedIn,
    shareId: snapshot.shareId,
  });
  const comments = collectSharedNoteComments(commentsQuery.data);
  const commentsAvailable = hasSharedNoteCollaborationAccess(
    commentsQuery.data?.pages[0],
  );
  const featuredAudio = findFeaturedSharedNoteAudio(snapshot.attachments);
  const canComposeComments =
    authenticatedNote !== null &&
    canComposeSharedNoteComments({
      capability: authenticatedNote.capability,
      hasCollaborationAccess: true,
      manageAccess: authenticatedNote.manageAccess,
    });

  return (
    <>
      <SharedNoteViewer
        accessLabel={accessLabel}
        actions={actions}
        collaboration={collaboration}
        documentContent={
          <SharedNoteReader
            canCompose={canComposeComments}
            excludedAttachmentIds={
              featuredAudio ? [featuredAudio.id] : undefined
            }
            manageAccess={authenticatedNote?.manageAccess ?? false}
            resolveAttachment={resolveAttachment}
            shareId={snapshot.shareId}
            signedIn={signedIn}
            snapshot={snapshot}
          />
        }
        headerActions={
          commentsAvailable ? (
            <SharedNoteCommentsDrawer comments={comments} />
          ) : undefined
        }
        meetingMetadata={meetingMetadata}
        resolveAttachment={resolveAttachment}
        snapshot={snapshot}
      />
      {chat?.(snapshot)}
    </>
  );
}
