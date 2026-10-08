import {
  computeCurrentNoteTab,
  hasSummaryContent,
  type SessionNoteView,
} from "@anlg/utils/session";

import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { useEnhancedNoteRecords, useSession } from "~/session/queries";
import { useListener } from "~/stt/contexts";

// `noteView` is undefined when the active tab is not a session, and null when
// the session tab still shows its default view.
export function useHasEditableSummary(
  sessionId: string | undefined,
  noteView: SessionNoteView | null | undefined,
) {
  const notes = useEnhancedNoteRecords(sessionId ?? "");
  const session = useSession(sessionId ?? "");
  const isEnhancing = useIsSessionEnhancing(sessionId ?? "");
  const isLiveSessionActive = useListener((state) =>
    sessionId ? state.getSessionMode(sessionId) === "active" : false,
  );
  return (
    !isEnhancing &&
    noteView !== undefined &&
    canEditSummary(notes, noteView, isLiveSessionActive, session?.title)
  );
}

// Only offered while a summary tab is shown, so the rewrite targets what the
// user is looking at.
export function canEditSummary(
  notes: ReadonlyArray<{ id: string; templateId: string; content: string }>,
  noteView: SessionNoteView | null,
  isLiveSessionActive: boolean,
  sessionTitle: string | undefined,
): boolean {
  const view = computeCurrentNoteTab(
    noteView,
    isLiveSessionActive,
    notes.map((note) => note.id),
  );
  const target =
    view.type === "enhanced"
      ? notes.find((note) => note.id === view.id)
      : undefined;
  if (!target) {
    return false;
  }
  // Without an explicit view, `edit_summary` falls back to the first
  // non-template summary (or the only summary), so only offer rewrites when
  // that is what is shown.
  if (noteView === null && target.templateId && notes.length > 1) {
    return false;
  }
  return hasSummaryContent(target.content, sessionTitle);
}
