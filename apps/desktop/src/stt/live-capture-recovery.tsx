import { useCallback, useEffect, useState } from "react";

import { commands as listenerCommands } from "@anlg/plugin-transcription";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import {
  hasPendingZeroRetentionAudio,
  loadCaptureLifecycleMarker,
} from "./capture-lifecycle-storage";
import { listenCaptureRecoveryRequests } from "./capture-recovery-requests";
import { useResumeListeningLifecycle } from "./useStartListening";

const CAPTURE_RECOVERY_BASE_RETRY_MS = 2_000;
const CAPTURE_RECOVERY_MAX_ATTEMPTS = 5;
const PENDING_AUDIO_RETRY_MS = 5 * 60_000;
const NATIVE_STOP_POLL_MS = 5_000;

async function isCapturing(sessionId: string) {
  try {
    const result = await listenerCommands.getCaptureSnapshot();
    return (
      result.status === "ok" &&
      (result.data.activeSessionId === sessionId ||
        result.data.finalizingSessionIds.includes(sessionId))
    );
  } catch {
    return false;
  }
}

async function hasPendingAudio(sessionId: string) {
  try {
    const marker = await loadCaptureLifecycleMarker(sessionId);
    return Boolean(marker && hasPendingZeroRetentionAudio(marker));
  } catch {
    return false;
  }
}

export function LiveCaptureRecovery() {
  const [recoveryTokens, setRecoveryTokens] = useState<
    Record<string, { token: number; processStopped: boolean }>
  >({});
  const completeRecovery = useCallback(
    (sessionId: string, recoveryToken: number) => {
      setRecoveryTokens((current) => {
        if (current[sessionId]?.token !== recoveryToken) {
          return current;
        }
        const next = { ...current };
        delete next[sessionId];
        return next;
      });
    },
    [],
  );

  useMountEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    // Explicit requests and unacknowledged native stop outcomes are processed;
    // marker-only captures found at launch wait for the user.
    const addSessionIds = (ids: Array<string | null>, requested = false) => {
      if (!active) {
        return;
      }
      setRecoveryTokens((current) => {
        const next = { ...current };
        for (const sessionId of ids) {
          if (!sessionId) {
            continue;
          }
          if (requested || !(sessionId in next)) {
            next[sessionId] = {
              token: (next[sessionId]?.token ?? 0) + 1,
              processStopped: requested,
            };
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
      .listCaptureRecoveries()
      .then((result) => {
        if (result.status === "error") {
          console.error(
            "[listener] failed to list capture recoveries",
            result.error,
          );
          return;
        }
        addSessionIds(
          result.data
            .filter((recovery) => recovery.process_stopped)
            .map((recovery) => recovery.session_id),
          true,
        );
        addSessionIds(
          result.data
            .filter((recovery) => !recovery.process_stopped)
            .map((recovery) => recovery.session_id),
        );
      })
      .catch((error) => {
        console.error("[listener] failed to list capture recoveries", error);
      });

    return () => {
      active = false;
      unlisten?.();
    };
  });

  return Object.entries(recoveryTokens).map(
    ([sessionId, { token, processStopped }]) => (
      <LiveCaptureSessionRecovery
        key={`${sessionId}:${token}`}
        sessionId={sessionId}
        recoveryToken={token}
        processStopped={processStopped}
        onComplete={completeRecovery}
      />
    ),
  );
}

function LiveCaptureSessionRecovery({
  sessionId,
  recoveryToken,
  processStopped,
  onComplete,
}: {
  sessionId: string;
  recoveryToken: number;
  processStopped: boolean;
  onComplete: (sessionId: string, recoveryToken: number) => void;
}) {
  const resumeListeningLifecycle = useResumeListeningLifecycle(sessionId);

  useEffect(() => {
    let active = true;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;

    const recover = async (attempt: number) => {
      let result: Awaited<ReturnType<typeof resumeListeningLifecycle>>;
      try {
        result = await resumeListeningLifecycle({
          abandonOnFailure: attempt >= CAPTURE_RECOVERY_MAX_ATTEMPTS,
          processStopped,
        });
      } catch (error) {
        console.error("[listener] capture recovery attempt failed", error);
        result = "error";
      }
      if (!active) {
        return;
      }
      if (result === "error") {
        if (
          attempt >= CAPTURE_RECOVERY_MAX_ATTEMPTS &&
          (await isCapturing(sessionId))
        ) {
          // Saved audio is offered only after the recording ends, and no
          // stop handler is attached to this capture.
          const waitForStop = async () => {
            if (!active) return;
            const capturing = await isCapturing(sessionId);
            if (!active) return;
            if (capturing) {
              retryTimer = setTimeout(waitForStop, NATIVE_STOP_POLL_MS);
              return;
            }
            void recover(attempt + 1);
          };
          if (active) retryTimer = setTimeout(waitForStop, NATIVE_STOP_POLL_MS);
          return;
        }
        if (
          attempt >= CAPTURE_RECOVERY_MAX_ATTEMPTS &&
          (await hasPendingAudio(sessionId))
        ) {
          if (!active) {
            return;
          }
          // Zero-retention audio is kept only until transcription succeeds.
          const retry = async () => {
            if (!active) return;
            // A new capture in this note adopts the pending audio.
            if (await isCapturing(sessionId)) {
              if (active)
                retryTimer = setTimeout(retry, PENDING_AUDIO_RETRY_MS);
              return;
            }
            void recover(attempt + 1);
          };
          retryTimer = setTimeout(retry, PENDING_AUDIO_RETRY_MS);
          return;
        }
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
  }, [
    onComplete,
    processStopped,
    recoveryToken,
    resumeListeningLifecycle,
    sessionId,
  ]);

  return null;
}
