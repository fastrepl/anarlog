import { useCallback } from "react";

import { useListener } from "./contexts";
import { useStartListening } from "./useStartListening";

export function useStartListeningWithBatchOverride(sessionId: string) {
  const startListening = useStartListening(sessionId);
  const { getSessionMode, stopTranscription } = useListener((state) => ({
    getSessionMode: state.getSessionMode,
    stopTranscription: state.stopTranscription,
  }));

  return useCallback(async () => {
    if (getSessionMode(sessionId) === "running_batch") {
      await stopTranscription(sessionId);
    }

    await startListening();
  }, [getSessionMode, sessionId, startListening, stopTranscription]);
}
