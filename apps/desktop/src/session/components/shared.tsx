import { useMemo } from "react";

import { computeCurrentNoteTab } from "./compute-note-tab";

import { extractPlainText } from "~/search/contexts/engine/utils";
import {
  useEnhancedNote,
  useEnhancedNoteRecords,
  useSession,
  useSessionHasTranscript,
} from "~/session/queries";
import type { SessionMode } from "~/store/zustand/listener/general";
import type { Tab } from "~/store/zustand/tabs/schema";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export { computeCurrentNoteTab } from "./compute-note-tab";

export function useHasTranscript(sessionId: string): boolean {
  return useSessionHasTranscript(sessionId);
}

export function hasStoredNoteContent(value: unknown): boolean {
  return extractPlainText(value).trim().length > 0;
}

export function useCurrentNoteHasContent(
  sessionId: string,
  currentView: EditorView,
): boolean {
  const hasTranscript = useHasTranscript(sessionId);
  const rawMd = useSession(sessionId)?.raw_md;
  const enhancedNoteId = currentView.type === "enhanced" ? currentView.id : "";
  const enhancedContent = useEnhancedNote(enhancedNoteId)?.content;

  if (currentView.type === "enhanced") {
    return hasStoredNoteContent(enhancedContent);
  }

  if (currentView.type === "transcript") {
    return hasTranscript;
  }

  return hasStoredNoteContent(rawMd);
}

export function useCurrentNoteTab(
  tab: Extract<Tab, { type: "sessions" }>,
  { audioExists = false }: { audioExists?: boolean } = {},
): EditorView {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const isLiveSessionActive = sessionMode === "active";
  const canShowTranscript = useCanShowTranscript(tab.id, { audioExists });

  const enhancedNoteIds = useEnhancedNoteRecords(tab.id).map((note) => note.id);

  return useMemo(() => {
    return computeCurrentNoteTab(
      tab.state.view ?? null,
      isLiveSessionActive,
      enhancedNoteIds,
      canShowTranscript,
    );
  }, [tab.state.view, isLiveSessionActive, enhancedNoteIds, canShowTranscript]);
}

export function useCanShowTranscript(
  sessionId: string,
  { audioExists = false }: { audioExists?: boolean } = {},
): boolean {
  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener((state) => state.batch[sessionId]?.error);
  const hasLiveSegments = useListener(
    (state) =>
      state.live.sessionId === sessionId && state.liveSegments.length > 0,
  );

  return getCanShowTranscript({
    audioExists,
    batchError: Boolean(batchError),
    hasLiveSegments,
    hasTranscript,
    sessionMode,
  });
}

export function getCanShowTranscript({
  audioExists = false,
  batchError = false,
  hasLiveSegments = false,
  hasTranscript,
  sessionMode,
}: {
  audioExists?: boolean;
  batchError?: boolean;
  hasLiveSegments?: boolean;
  hasTranscript: boolean;
  sessionMode: SessionMode;
}): boolean {
  const isLiveCapture =
    sessionMode === "active" || sessionMode === "finalizing";

  return (
    hasTranscript ||
    (audioExists && !isLiveCapture) ||
    isLiveCapture ||
    hasLiveSegments ||
    sessionMode === "running_batch" ||
    Boolean(batchError)
  );
}

export function RecordingIcon() {
  return (
    <span className="relative flex size-3 items-center justify-center">
      <span className="absolute size-2.5 animate-ping rounded-full bg-red-500/40" />
      <span className="relative size-2 rounded-full bg-red-500" />
    </span>
  );
}
