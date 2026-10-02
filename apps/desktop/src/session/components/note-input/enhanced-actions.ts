import { useCallback } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { toast } from "@anlg/ui/components/ui/toast";

import { useAITaskTask } from "~/ai/hooks";
import { useLanguageModel } from "~/ai/hooks";
import {
  isMainAITaskHostWindow,
  requestMainAITaskCancel,
  requestMainEnhance,
} from "~/ai/task-window-sync";
import { getEnhancerService } from "~/services/enhancer";
import { getEligibility } from "~/services/enhancer/eligibility";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import { useEnhancedNote } from "~/session/queries";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export function useEnhancedNoteActions({
  enhancedNoteId,
  sessionId,
}: {
  enhancedNoteId: string | null;
  sessionId: string;
}) {
  const model = useLanguageModel("enhance");
  const taskId = enhancedNoteId
    ? createTaskId(enhancedNoteId, "enhance")
    : null;

  const note = useEnhancedNote(enhancedNoteId ?? "");
  const noteTemplateId = note?.templateId || undefined;

  const enhanceTask = useAITaskTask(taskId, "enhance");

  const onRegenerate = useCallback(
    async (templateId: string | null) => {
      if (!enhancedNoteId) {
        return;
      }

      if (!model) {
        toast.error(
          "Set up Intelligence in Settings before regenerating this summary.",
        );
        return;
      }

      const snapshot = await loadSessionContentSnapshot(sessionId);
      if (snapshot) {
        const eligibility = getEligibility(snapshot.transcripts);
        if (
          !eligibility.eligible &&
          eligibility.code === "transcript_too_short"
        ) {
          toast.warning("Summary wasn't generated", {
            id: `auto-summary-too-short-${sessionId}`,
            description: eligibility.reason,
          });
          return;
        }
      }

      if (!isMainAITaskHostWindow()) {
        void requestMainEnhance(sessionId, {
          templateId: templateId ?? noteTemplateId,
          targetNoteId: note ? enhancedNoteId : undefined,
        });
        return;
      }

      void analyticsCommands.event({
        event: "note_enhanced",
        is_auto: false,
      });

      if (!note) {
        await getEnhancerService()?.enhance(sessionId, {
          templateId: templateId ?? noteTemplateId,
        });
        return;
      }

      await enhanceTask.start({
        model,
        args: {
          sessionId,
          enhancedNoteId,
          templateId: templateId ?? noteTemplateId,
        },
      });
    },
    [enhancedNoteId, model, enhanceTask.start, sessionId, noteTemplateId, note],
  );

  const onCancel = useCallback(() => {
    if (!taskId) {
      return;
    }

    if (!isMainAITaskHostWindow()) {
      void requestMainAITaskCancel(taskId);
      return;
    }

    enhanceTask.cancel();
  }, [enhanceTask.cancel, taskId]);

  return {
    isGenerating: enhanceTask.isGenerating,
    isError: enhanceTask.isError,
    error: enhanceTask.error,
    onRegenerate,
    onCancel,
  };
}
