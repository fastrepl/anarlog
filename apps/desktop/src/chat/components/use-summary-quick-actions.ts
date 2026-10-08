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
  notes: ReadonlyArray<{ id: string; content: string }>,
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
  return target ? hasSummaryContent(target.content, sessionTitle) : false;
}
