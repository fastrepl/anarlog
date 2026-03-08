import { useEffect, useMemo } from "react";

import { useHasTranscript } from "../components/shared";

import { useAITask } from "~/ai/contexts";
import { getEnhancerService } from "~/services/enhancer";
import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { useListener } from "~/stt/contexts";

export function useEnhancedNotes(sessionId: string) {
  return main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );
}

export function useEnhancedNote(enhancedNoteId: string) {
  const title = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "title",
    main.STORE_ID,
  );
  const content = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "content",
    main.STORE_ID,
  );
  const position = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "position",
    main.STORE_ID,
  );
  const templateId = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "template_id",
    main.STORE_ID,
  );

  return { title, content, position, templateId };
}

export function useEnsureDefaultSummary(sessionId: string) {
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const hasTranscript = useHasTranscript(sessionId);
  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );
  const selectedTemplateId = settings.UI.useValue(
    "selected_template_id",
    settings.STORE_ID,
  ) as string | undefined;

  useEffect(() => {
    if (
      !hasTranscript ||
      sessionMode === "active" ||
      sessionMode === "running_batch" ||
      sessionMode === "finalizing" ||
      (enhancedNoteIds && enhancedNoteIds.length > 0)
    ) {
      return;
    }

    getEnhancerService()?.ensureNote(
      sessionId,
      selectedTemplateId || undefined,
    );
  }, [
    hasTranscript,
    sessionMode,
    sessionId,
    enhancedNoteIds?.length,
    selectedTemplateId,
  ]);
}

export function useIsSessionEnhancing(sessionId: string): boolean {
  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );

  const taskIds = useMemo(
    () => (enhancedNoteIds || []).map((id) => createTaskId(id, "enhance")),
    [enhancedNoteIds],
  );

  const isEnhancing = useAITask((state) => {
    return taskIds.some(
      (taskId) => state.tasks[taskId]?.status === "generating",
    );
  });

  return isEnhancing;
}
