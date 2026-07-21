import { lazy, Suspense, useState } from "react";

import {
  type SharedAttachmentResolver,
  SharedNoteDocument,
} from "@/components/shared-note-document";
import { useMountEffect } from "@/hooks/useMountEffect";
import { hasUnsupportedSharedNoteEditorNode } from "@/lib/shared-note-editing";
import {
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

const SharedNoteReadSurface = lazy(() =>
  import("@/components/shared-note-read-surface").then((module) => ({
    default: module.SharedNoteReadSurface,
  })),
);

export function SharedNoteReader({
  canCompose,
  manageAccess,
  resolveAttachment,
  shareId,
  signedIn,
  snapshot,
}: {
  canCompose: boolean;
  manageAccess: boolean;
  resolveAttachment?: SharedAttachmentResolver;
  shareId: string;
  signedIn: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const [interactive, setInteractive] = useState(false);
  useMountEffect(() => setInteractive(true));

  const staticDocument = (
    <SharedNoteDocument
      attachments={snapshot.attachments}
      document={withoutDuplicateLeadingTitle(snapshot.body, snapshot.title)}
      resolveAttachment={resolveAttachment}
    />
  );

  if (!interactive || hasUnsupportedSharedNoteEditorNode(snapshot.body)) {
    return staticDocument;
  }

  return (
    <Suspense fallback={staticDocument}>
      <SharedNoteReadSurface
        key={`${snapshot.shareId}:${snapshot.contentRevision}`}
        canCompose={canCompose}
        manageAccess={manageAccess}
        resolveAttachment={resolveAttachment}
        shareId={shareId}
        signedIn={signedIn}
        snapshot={snapshot}
      />
    </Suspense>
  );
}
