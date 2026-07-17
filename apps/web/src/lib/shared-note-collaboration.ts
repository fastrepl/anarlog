import type { SharedNoteCapability } from "@/lib/shared-notes";

export const MAX_SHARED_NOTE_COMMENT_BYTES = 16_384;

export function validateSharedNoteCommentBody(value: string) {
  const body = value.trim();
  const byteLength = new TextEncoder().encode(body).byteLength;
  return {
    body,
    byteLength,
    valid: body.length > 0 && byteLength <= MAX_SHARED_NOTE_COMMENT_BYTES,
  };
}

export function formatAuthenticatedSharedNoteAccessLabel({
  capability,
  manageAccess,
}: {
  capability: SharedNoteCapability;
  manageAccess: boolean;
}) {
  if (manageAccess) return "You manage this note · Can edit and comment";
  if (capability === "editor") return "Shared with you · Can edit and comment";
  if (capability === "commenter") return "Shared with you · Can comment";
  return "Shared with you · View only";
}

export function shouldUseAuthenticatedSharedNoteAccessLabel({
  capability,
  manageAccess,
}: {
  capability: SharedNoteCapability;
  manageAccess: boolean;
}) {
  return manageAccess || capability !== "viewer";
}

export function canComposeSharedNoteComments({
  capability,
  hasCollaborationAccess,
  manageAccess,
}: {
  capability: SharedNoteCapability;
  hasCollaborationAccess: boolean;
  manageAccess: boolean;
}) {
  return (
    hasCollaborationAccess &&
    (manageAccess || capability === "commenter" || capability === "editor")
  );
}

export function hasSharedNoteCollaborationAccess(
  result:
    | { status: "ready" }
    | { status: "unavailable" }
    | { status: "error" }
    | undefined,
) {
  return result?.status === "ready";
}

export function formatSharedNoteAccessRequestDescription(
  capability: SharedNoteCapability,
) {
  if (capability === "editor") return "Requested permission to edit";
  if (capability === "commenter") return "Requested permission to comment";
  return "Requested permission to view";
}
