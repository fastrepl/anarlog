import { useCallback, useEffect, useState } from "react";

import { commands as listenerCommands } from "@hypr/plugin-transcription";

import { loadCaptureLifecycleMarkers } from "./capture-lifecycle-storage";
import { listenCaptureRecoveryRequests } from "./capture-recovery-requests";
import { useResumeListeningLifecycle } from "./useStartListening";

const CAPTURE_RECOVERY_RETRY_MS = 2_000;

export function LiveCaptureRecovery() {
  const [recoveryTokens, setRecoveryTokens] = useState<Record<string, number>>(
    {},
  );
  const completeRecovery = useCallback(
    (sessionId: string, recoveryToken: number) => {
      setRecoveryTokens((current) => {
        if (current[sessionId] !== recoveryToken) {
          return current;
        }
        const next = { ...current };
        delete next[sessionId];
        return next;
      });
    },
    [],
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const addSessionIds = (
      ids: Array<string | null>,
      restartExisting = false,
    ) => {
      if (!active) {
        return;
      }
      setRecoveryTokens((current) => {
        const next = { ...current };
        for (const sessionId of ids) {
          if (!sessionId) {
            continue;
          }
          if (restartExisting || !(sessionId in next)) {
            next[sessionId] = (next[sessionId] ?? 0) + 1;
          }
        }
        return next;
      });
    };

    void listenCaptureRecoveryRequests((sessionId) => {
      addSessionIds([sessionId], true);
    })
      .then((stopListening) => {
        if (!active) {
          stopListening();
          return;
        }
        unlisten = stopListening;
      })
      .catch((error) => {
        console.error(
          "[listener] failed to listen for capture recovery requests",
          error,
        );
      });

    void listenerCommands
      .getCaptureSnapshot()
      .then((result) => {
        if (result.status === "error") {
          console.error(
            "[listener] failed to recover active capture:",
            result.error,
          );
          return;
        }
        addSessionIds([
          result.data.activeSessionId,
          ...result.data.finalizingSessionIds,
        ]);
      })
      .catch((error) => {
        console.error("[listener] failed to recover active capture:", error);
      });

    void loadCaptureLifecycleMarkers()
      .then((markers) => {
        addSessionIds(markers.map((marker) => marker.sessionId));
      })
      .catch((error) => {
        console.error(
          "[listener] failed to load capture recovery state",
          error,
        );
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  return Object.entries(recoveryTokens).map(([sessionId, recoveryToken]) => (
    <LiveCaptureSessionRecovery
      key={`${sessionId}:${recoveryToken}`}
      sessionId={sessionId}
      recoveryToken={recoveryToken}
      onComplete={completeRecovery}
    />
  ));
}

function LiveCaptureSessionRecovery({
  sessionId,
  recoveryToken,
  onComplete,
}: {
  sessionId: string;
  recoveryToken: number;
  onComplete: (sessionId: string, recoveryToken: number) => void;
}) {
  const resumeListeningLifecycle = useResumeListeningLifecycle(sessionId);

  useEffect(() => {
    let active = true;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;

    const recover = async () => {
      let result: "attached" | "inactive" | "error";
      try {
        result = await resumeListeningLifecycle();
      } catch (error) {
        console.error("[listener] capture recovery attempt failed", error);
        result = "error";
      }
      if (!active) {
        return;
      }
      if (result === "error") {
        retryTimer = setTimeout(() => {
          void recover();
        }, CAPTURE_RECOVERY_RETRY_MS);
        return;
      }
      onComplete(sessionId, recoveryToken);
    };

    void recover();

    return () => {
      active = false;
      if (retryTimer) {
        clearTimeout(retryTimer);
      }
    };
  }, [onComplete, recoveryToken, resumeListeningLifecycle, sessionId]);

  return null;
}
