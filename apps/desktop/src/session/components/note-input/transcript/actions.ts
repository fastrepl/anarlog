import { t } from "@lingui/core/macro";
import { useCallback } from "react";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { withCloudsyncActivity } from "~/db/cloudsync-activity";
import { getEnhancerService } from "~/services/enhancer";
import { useListener } from "~/stt/contexts";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";

export function useRegenerateTranscript(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);

  return useCallback(async () => {
    const result = await fsSyncCommands.audioPath(sessionId);
    if (result.status === "error") {
      sonnerToast.error(t`Recording not found. It may have been deleted.`, {
        id: `transcript-regenerate-audio-missing-${sessionId}`,
      });
      return;
    }

    const audioPath = result.data;

    try {
      await withCloudsyncActivity(
        "transcription",
        `${sessionId}:retranscription:${crypto.randomUUID()}`,
        async () => {
          await runBatch(audioPath, {
            promotion: { scope: "whole_session" },
          });
          await getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId);
        },
      );
    } catch (error) {
      if (isStoppedTranscriptionError(error)) {
        return;
      }
      const msg = error instanceof Error ? error.message : String(error);
      handleBatchFailed(sessionId, msg);
      sonnerToast.error(t`Re-transcription failed`, {
        id: `transcript-regenerate-failed-${sessionId}`,
        description: msg,
      });
    }
  }, [handleBatchFailed, runBatch, sessionId]);
}
