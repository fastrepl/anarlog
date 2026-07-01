import { useCallback, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { useAITaskTask } from "~/ai/hooks";
import { useLanguageModel, useLLMConnectionStatus } from "~/ai/hooks";
import {
  isMainAITaskHostWindow,
  requestMainAITaskCancel,
  requestMainEnhance,
} from "~/ai/task-window-sync";
import { shouldShowEmptySummaryConfigError } from "~/session/enhance-config";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export function useEnhancedNoteActions({
  enhancedNoteId,
  sessionId,
}: {
  enhancedNoteId: string | null;
  sessionId: string;
}) {
  const model = useLanguageModel("enhance");
  const llmStatus = useLLMConnectionStatus();
  const taskId = enhancedNoteId
    ? createTaskId(enhancedNoteId, "enhance")
    : null;
  const [missingModelError, setMissingModelError] = useState<Error | null>(
    null,
  );

  const noteTemplateId =
    (main.UI.useCell(
      "enhanced_notes",
      enhancedNoteId ?? "",
      "template_id",
      main.STORE_ID,
    ) as string | undefined) || undefined;

  const enhanceTask = useAITaskTask(taskId, "enhance");

  const onRegenerate = useCallback(
    async (templateId: string | null) => {
      if (!enhancedNoteId) {
        return;
      }

      if (!model) {
        setMissingModelError(
          new Error("Intelligence provider not configured."),
        );
        return;
      }

      setMissingModelError(null);

      if (!isMainAITaskHostWindow()) {
        void requestMainEnhance(sessionId, {
          templateId: templateId ?? noteTemplateId,
          targetNoteId: enhancedNoteId,
        });
        return;
      }

      void analyticsCommands.event({
        event: "note_enhanced",
        is_auto: false,
      });

      await enhanceTask.start({
        model,
        args: {
          sessionId,
          enhancedNoteId,
          templateId: templateId ?? noteTemplateId,
        },
      });
    },
    [enhancedNoteId, model, enhanceTask.start, sessionId, noteTemplateId],
  );

  const isConfigError = shouldShowEmptySummaryConfigError(llmStatus);
  const isIdleWithConfigError = enhanceTask.isIdle && isConfigError;
  const error = model ? enhanceTask.error : missingModelError;
  const isError = !!error || enhanceTask.isError || isIdleWithConfigError;
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
    isError,
    error,
    onRegenerate,
    onCancel,
  };
}
