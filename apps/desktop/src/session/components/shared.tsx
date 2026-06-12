import { useMemo } from "react";

import { Button } from "@hypr/ui/components/ui/button";

import { computeCurrentNoteTab } from "./compute-note-tab";

import { extractPlainText } from "~/search/contexts/engine/utils";
import * as main from "~/store/tinybase/store/main";
import type { Tab } from "~/store/zustand/tabs/schema";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { parseTranscriptWords } from "~/stt/utils";

export { computeCurrentNoteTab } from "./compute-note-tab";

export function useHasTranscript(sessionId: string): boolean {
  const transcriptIds =
    main.UI.useSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId,
      main.STORE_ID,
    ) ?? [];
  const transcriptsTable = main.UI.useTable("transcripts", main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);

  return useMemo(() => {
    if (!store) {
      return false;
    }

    return transcriptIds.some(
      (transcriptId) => parseTranscriptWords(store, transcriptId).length > 0,
    );
  }, [store, transcriptIds, transcriptsTable]);
}

export function hasStoredNoteContent(value: unknown): boolean {
  return extractPlainText(value).trim().length > 0;
}

export function useCurrentNoteHasContent(
  sessionId: string,
  currentView: EditorView,
): boolean {
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const enhancedNoteId = currentView.type === "enhanced" ? currentView.id : "";
  const enhancedContent = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "content",
    main.STORE_ID,
  );

  if (currentView.type === "enhanced") {
    return hasStoredNoteContent(enhancedContent);
  }

  return hasStoredNoteContent(rawMd);
}

export function useCurrentNoteTab(
  tab: Extract<Tab, { type: "sessions" }>,
): EditorView {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const isLiveSessionActive = sessionMode === "active";

  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    tab.id,
    main.STORE_ID,
  );
  const firstEnhancedNoteId = enhancedNoteIds?.[0];

  return useMemo(
    () =>
      computeCurrentNoteTab(
        tab.state.view ?? null,
        isLiveSessionActive,
        firstEnhancedNoteId,
      ),
    [tab.state.view, isLiveSessionActive, firstEnhancedNoteId],
  );
}

export function RecordingIcon() {
  return <div className="size-3 rounded-full bg-red-500" />;
}

export function useListenButtonState(sessionId: string) {
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const lastError = useListener((state) => state.live.lastError);
  const active = sessionMode === "active" || sessionMode === "finalizing";
  const batching = sessionMode === "running_batch";

  const shouldRender = !active;
  const isDisabled = batching;

  let warningMessage = "";
  if (lastError) {
    warningMessage = `Session failed: ${lastError}`;
  } else if (batching) {
    warningMessage = "Batch transcription in progress.";
  }

  return {
    shouldRender,
    isDisabled,
    warningMessage,
  };
}

export function ActionableTooltipContent({
  message,
  action,
}: {
  message: string;
  action?: {
    label: string;
    handleClick: () => void;
  };
}) {
  return (
    <div className="flex flex-row items-center gap-3">
      <p className="text-xs">{message}</p>
      {action && (
        <Button
          size="sm"
          variant="outline"
          className="text-foreground rounded-md"
          onClick={action.handleClick}
        >
          {action.label}
        </Button>
      )}
    </div>
  );
}
