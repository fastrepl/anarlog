import { useCallback, useEffect, useState } from "react";

import { commands as listenerCommands } from "@anlg/plugin-transcription";

import { loadCaptureLifecycleMarkers } from "./capture-lifecycle-storage";
import { listenCaptureRecoveryRequests } from "./capture-recovery-requests";
import { useResumeListeningLifecycle } from "./useStartListening";

const CAPTURE_RECOVERY_BASE_RETRY_MS = 2_000;
const CAPTURE_RECOVERY_MAX_ATTEMPTS = 5;

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

    const recover = async (attempt: number) => {
      let result: "attached" | "inactive" | "error";
      try {
        result = await resumeListeningLifecycle({
          abandonOnFailure: attempt >= CAPTURE_RECOVERY_MAX_ATTEMPTS,
        });
      } catch (error) {
        console.error("[listener] capture recovery attempt failed", error);
        result = "error";
      }
      if (!active) {
        return;
      }
      if (result === "error") {
        if (attempt >= CAPTURE_RECOVERY_MAX_ATTEMPTS) {
          console.warn("[listener] capture recovery retry budget exhausted", {
            sessionId,
          });
          onComplete(sessionId, recoveryToken);
          return;
        }
        retryTimer = setTimeout(
          () => {
            void recover(attempt + 1);
          },
          CAPTURE_RECOVERY_BASE_RETRY_MS * 2 ** (attempt - 1),
        );
        return;
      }
      onComplete(sessionId, recoveryToken);
    };

    void recover(1);

    return () => {
      active = false;
      if (retryTimer) {
        clearTimeout(retryTimer);
      }
    };
  }, [onComplete, recoveryToken, resumeListeningLifecycle, sessionId]);

  return null;
}
