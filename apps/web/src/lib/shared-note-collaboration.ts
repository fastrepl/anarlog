import type { SharedNoteCapability } from "@/lib/shared-notes";

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

export function canComposeSharedNoteComments({
  capability,
  hasAuthenticatedAccess,
  manageAccess,
}: {
  capability: SharedNoteCapability;
  hasAuthenticatedAccess: boolean;
  manageAccess: boolean;
}) {
  return (
    hasAuthenticatedAccess &&
    (manageAccess || capability === "commenter" || capability === "editor")
  );
}
