import { useEffect, useMemo } from "react";

import { useAITask } from "~/ai/contexts";
import { getEnhancerService } from "~/services/enhancer";
import { useHasTranscript } from "~/session/components/shared";
import {
  useEnhancedNote as useSqliteEnhancedNote,
  useEnhancedNoteRecords,
} from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import type { SessionMode } from "~/store/zustand/listener/general";
import { useListener } from "~/stt/contexts";

export function useEnhancedNotes(sessionId: string) {
  const notes = useEnhancedNoteRecords(sessionId);
  return useMemo(() => notes.map((note) => note.id), [notes]);
}

export function useEnhancedNote(enhancedNoteId: string) {
  const note = useSqliteEnhancedNote(enhancedNoteId);

  return {
    title: note?.title,
    content: note?.content,
    position: note?.position,
    templateId: note?.templateId,
  };
}

export function useEnsureDefaultSummary(sessionId: string) {
  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener((state) => state.batch[sessionId]?.error);
  const enhancedNoteIds = useEnhancedNotes(sessionId);
  useEnsureDefaultSummaryFromState({
    batchError: Boolean(batchError),
    enhancedNoteCount: enhancedNoteIds.length,
    hasTranscript,
    sessionId,
    sessionMode,
  });
}

export function useEnsureDefaultSummaryFromState({
  batchError,
  enabled = true,
  enhancedNoteCount,
  hasTranscript,
  sessionId,
  sessionMode,
}: {
  batchError: boolean;
  enabled?: boolean;
  enhancedNoteCount: number;
  hasTranscript: boolean;
  sessionId: string;
  sessionMode: SessionMode;
}) {
  const selectedTemplateId = useConfigValue("selected_template_id");
  const templateId = selectedTemplateId || undefined;

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const service = getEnhancerService();
    if (!service) {
      return;
    }

    const isLiveCapture =
      sessionMode === "active" || sessionMode === "finalizing";
    const canCreateSummary =
      !isLiveCapture &&
      (hasTranscript || sessionMode === "running_batch" || batchError);

    if (enhancedNoteCount === 0 && canCreateSummary) {
      void Promise.resolve(service.ensureNote(sessionId, templateId)).catch(
        (error) => {
          console.error("[enhancer] failed to create default summary", error);
        },
      );
    }
  }, [
    sessionId,
    enhancedNoteCount,
    templateId,
    hasTranscript,
    sessionMode,
    batchError,
    enabled,
  ]);
}

export function useIsSessionEnhancing(sessionId: string): boolean {
  const enhancedNoteIds = useEnhancedNotes(sessionId);

  const taskIds = useMemo(
    () => enhancedNoteIds.map((id) => createTaskId(id, "enhance")),
    [enhancedNoteIds],
  );

  const isEnhancing = useAITask((state) => {
    return taskIds.some(
      (taskId) => state.tasks[taskId]?.status === "generating",
    );
  });

  return isEnhancing;
}
