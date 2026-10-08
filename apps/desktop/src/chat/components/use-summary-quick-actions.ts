import { hasSummaryContent } from "@anlg/utils/session";

import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { useEnhancedNoteRecords, useSession } from "~/session/queries";

export function useHasEditableSummary(
  sessionId: string | undefined,
  activeEnhancedNoteId: string | undefined,
) {
  const notes = useEnhancedNoteRecords(sessionId ?? "");
  const session = useSession(sessionId ?? "");
  const isEnhancing = useIsSessionEnhancing(sessionId ?? "");
  return (
    !isEnhancing && canEditSummary(notes, activeEnhancedNoteId, session?.title)
  );
}

// Only offered while a summary tab is open, so the rewrite targets what the
// user is looking at.
export function canEditSummary(
  notes: ReadonlyArray<{ id: string; content: string }>,
  activeEnhancedNoteId: string | undefined,
  sessionTitle: string | undefined,
): boolean {
  const target = activeEnhancedNoteId
    ? notes.find((note) => note.id === activeEnhancedNoteId)
    : undefined;
  return target ? hasSummaryContent(target.content, sessionTitle) : false;
}
