import { useCallback, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { useListener } from "./contexts";
import { useLiveTranscriptPersistence } from "./session-storage";
import { useKeywords } from "./useKeywords";
import {
  canRunBatchTranscription,
  isStoppedTranscriptionError,
  useRunBatch,
} from "./useRunBatch";
import { useSTTConnection } from "./useSTTConnection";

import { getEnhancerService } from "~/services/enhancer";
import { useCurrentUserId } from "~/session/hooks/storage";
import { useConfigValue } from "~/shared/config";
import type {
  LiveTranscriptPersistCallback,
  OnStoppedCallback,
} from "~/store/zustand/listener/transcript";

export function getPostCaptureAction(
  details: {
    audioPath: string | null;
    liveTranscriptionActive: boolean;
  },
  canRunBatch: boolean,
) {
  if (details.liveTranscriptionActive) {
    return "enhance_only" as const;
  }

  if (!!details.audioPath && canRunBatch) {
    return "batch_then_enhance" as const;
  }

  return "none" as const;
}

export function useStartListening(sessionId: string) {
  const user_id = useCurrentUserId();
  const {
    participantHumanIds,
    hasCalendarEvent,
    persistDelta,
    rollbackTranscript,
  } = useLiveTranscriptPersistence(sessionId);

  const languages = useConfigValue("spoken_languages");

  const start = useListener((state) => state.start);
  const { conn } = useSTTConnection();
  const runBatch = useRunBatch(sessionId);

  const keywords = useKeywords(sessionId);
  const runBatchRef = useRef(runBatch);
  const canRunBatchRef = useRef(canRunBatchTranscription(conn));
  runBatchRef.current = runBatch;
  canRunBatchRef.current = canRunBatchTranscription(conn);

  const startListening = useCallback(async () => {
    let transcriptId: string | null = null;
    const startedAt = Date.now();
    const createdAt = new Date().toISOString();

    const onStopped: OnStoppedCallback = async (_sessionId, details) => {
      const postCaptureAction = getPostCaptureAction(
        details,
        canRunBatchRef.current,
      );

      if (postCaptureAction === "batch_then_enhance") {
        try {
          await runBatchRef.current(details.audioPath!);
        } catch (error) {
          if (isStoppedTranscriptionError(error)) {
            return;
          }
          console.error(
            "[listener] failed to run post-capture transcription",
            error,
          );
          return;
        }
      }

      if (postCaptureAction === "none") {
        return;
      }

      getEnhancerService()?.queueAutoEnhance(sessionId);
    };

    const handlePersist: LiveTranscriptPersistCallback = (delta) => {
      if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
        return;
      }

      transcriptId = persistDelta({
        currentTranscriptId: transcriptId,
        startedAt,
        createdAt,
        delta,
      });
    };

    const started = await start(
      {
        session_id: sessionId,
        languages,
        onboarding: false,
        model: conn?.model ?? "",
        base_url: conn?.baseUrl ?? "",
        api_key: conn?.apiKey ?? "",
        keywords,
        participant_human_ids: participantHumanIds,
        self_human_id: typeof user_id === "string" ? user_id : null,
      },
      {
        handlePersist,
        onStopped,
      },
    );

    if (!started) {
      rollbackTranscript(transcriptId);
      return;
    }

    void analyticsCommands.event({
      event: "session_started",
      has_calendar_event: hasCalendarEvent,
      ...(conn
        ? {
            stt_provider: conn.provider,
            stt_model: conn.model,
          }
        : {}),
    });
  }, [
    conn,
    sessionId,
    start,
    keywords,
    user_id,
    languages,
    participantHumanIds,
    persistDelta,
    rollbackTranscript,
    hasCalendarEvent,
  ]);

  return startListening;
}
