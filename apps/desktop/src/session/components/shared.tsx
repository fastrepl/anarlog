import { useMemo } from "react";

import { Button } from "@hypr/ui/components/ui/button";

import { computeCurrentNoteTab } from "./compute-note-tab";

import { useAITaskTask } from "~/ai/hooks";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import type { Tab } from "~/store/zustand/tabs/schema";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export { computeCurrentNoteTab } from "./compute-note-tab";

export function useHasTranscript(sessionId: string): boolean {
  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );

  return !!transcriptIds && transcriptIds.length > 0;
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

  const taskId = createTaskId(sessionId, "enhance");
  const { status } = useAITaskTask(taskId, "enhance");
  const generating = status === "generating";

  const shouldRender = !active && !generating;
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
          className="rounded-md text-black"
          onClick={action.handleClick}
        >
          {action.label}
        </Button>
      )}
    </div>
  );
}
