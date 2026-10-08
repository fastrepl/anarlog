import { hasSummaryContent } from "@anlg/utils/session";

import { resolveSummaryTargetId } from "~/chat/tools/current-session";
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

export function canEditSummary(
  notes: ReadonlyArray<{ id: string; templateId: string; content: string }>,
  activeEnhancedNoteId: string | undefined,
  sessionTitle: string | undefined,
): boolean {
  const targetId = resolveSummaryTargetId(notes, activeEnhancedNoteId);
  const target = notes.find((note) => note.id === targetId);
  return target ? hasSummaryContent(target.content, sessionTitle) : false;
}
