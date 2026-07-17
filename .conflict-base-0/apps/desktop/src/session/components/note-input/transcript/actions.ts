import { useCallback } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { getEnhancerService } from "~/services/enhancer";
import { useListener } from "~/stt/contexts";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";

export function useRegenerateTranscript(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);

  return useCallback(async () => {
    const result = await fsSyncCommands.audioPath(sessionId);
    if (result.status === "error") {
      sonnerToast.error("Recording not found. It may have been deleted.", {
        id: `transcript-regenerate-audio-missing-${sessionId}`,
      });
      return;
    }

    const audioPath = result.data;

    try {
      await runBatch(audioPath);
      await getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId);
    } catch (error) {
      if (isStoppedTranscriptionError(error)) {
        return;
      }
      const msg = error instanceof Error ? error.message : String(error);
      handleBatchFailed(sessionId, msg);
    }
  }, [handleBatchFailed, runBatch, sessionId]);
}
