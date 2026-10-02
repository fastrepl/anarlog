import { useEffect, useRef, useState } from "react";

import type { TranscriptionSession } from "@anlg/plugin-transcription";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { getEnhancerService } from "~/services/enhancer";
import { useSession } from "~/session/queries";
import { recoverRunningBatchSessions } from "~/store/zustand/listener/general-batch";
import { listenerStore } from "~/store/zustand/listener/instance";
import { parseBatchResumeContext } from "~/stt/batch-resume-context";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";

export function BatchTranscriptionRecovery() {
  const [resumable, setResumable] = useState<TranscriptionSession[]>([]);

  useMountEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;

    void recoverRunningBatchSessions(listenerStore.getState, (sessions) => {
      if (active) setResumable(sessions);
    })
      .then((dispose) => {
        if (!active) {
          dispose();
          return;
        }
        stop = dispose;
      })
      .catch((error) => {
        console.error("[listener] failed to recover batch sessions", error);
      });

    return () => {
      active = false;
      stop?.();
    };
  });

  return (
    <>
      {resumable.map((session) => (
        <ResumeBatchTranscription key={session.session_id} session={session} />
      ))}
    </>
  );
}

function ResumeBatchTranscription({
  session,
}: {
  session: TranscriptionSession;
}) {
  const sessionId = session.session_id;
  const record = useSession(sessionId);
  const runBatch = useRunBatch(sessionId);
  const started = useRef(false);

  useEffect(() => {
    if (started.current || !record) return;
    const context = parseBatchResumeContext(session.resume_context);
    if (!context || !session.provider || !session.model) return;
    started.current = true;

    void runBatch(session.file_path, {
      promotion: { scope: context.promotion },
      resume: { provider: session.provider, model: session.model },
    })
      .then(() =>
        getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId),
      )
      .catch((error) => {
        if (isStoppedTranscriptionError(error)) return;
        const message = error instanceof Error ? error.message : String(error);
        console.error("[listener] failed to resume batch transcription", {
          sessionId,
          error: message,
        });
        listenerStore.getState().handleBatchFailed(sessionId, message);
      });
  }, [record, runBatch, session, sessionId]);

  return null;
}
