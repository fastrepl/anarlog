import { useCallback, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as detectCommands } from "@hypr/plugin-detect";

import { useListener } from "./contexts";
import { getSessionKeywords } from "./useKeywords";
import {
  canRunBatchTranscription,
  isStoppedTranscriptionError,
  useRunBatch,
} from "./useRunBatch";
import { useSTTConnection } from "./useSTTConnection";

import { useShell } from "~/contexts/shell";
import {
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
} from "~/services/audio-retention";
import { getEnhancerService } from "~/services/enhancer";
import { useSession, useSessionHasTranscript } from "~/session/queries";
import { getSessionEvent } from "~/session/utils";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import type {
  LiveTranscriptPersistCallback,
  OnStoppedCallback,
} from "~/store/zustand/listener/transcript";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";
import {
  applyLiveTranscriptDeltaToDatabase,
  createLiveTranscript,
  softDeleteTranscript,
  useSessionParticipantHumanIds,
} from "~/stt/queries";

const CONSENT_CHAT_MESSAGE =
  "I'm using Anarlog, a private meeting notepad, to record and transcribe this meeting. Learn more at https://anarlog.so. Please reply here if you do not consent.";

export async function sendConsentRequestToMeetingChat() {
  let micAppsResult: Awaited<
    ReturnType<typeof detectCommands.listMicUsingApplications>
  >;

  try {
    micAppsResult = await detectCommands.listMicUsingApplications();
  } catch (error) {
    console.warn("[listener] failed to find the mic-active meeting app", error);
    return;
  }

  if (micAppsResult.status === "error") {
    console.warn(
      "[listener] failed to find the mic-active meeting app",
      micAppsResult.error,
    );
    return;
  }

  const micActiveBundleIds = [
    ...new Set(micAppsResult.data.map((app) => app.id.trim()).filter(Boolean)),
  ];
  if (micActiveBundleIds.length === 0) {
    console.warn(
      "[listener] consent message was not sent",
      "no mic-active app was found",
    );
    return;
  }

  let result: Awaited<ReturnType<typeof detectCommands.sendMeetingChatMessage>>;

  try {
    result = await detectCommands.sendMeetingChatMessage(
      CONSENT_CHAT_MESSAGE,
      micActiveBundleIds,
    );
  } catch (error) {
    console.warn("[listener] failed to send consent message", error);
    return;
  }

  if (result.status === "error") {
    console.warn("[listener] failed to send consent message", result.error);
    return;
  }

  if (!result.data.sent) {
    console.warn(
      "[listener] consent message was not sent",
      result.data.warnings,
    );
  }
}

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
  const session = useSession(sessionId);
  const hadTranscriptBeforeStart = useSessionHasTranscript(sessionId);
  const participantHumanIds = useSessionParticipantHumanIds(sessionId);

  const aiLanguage = useConfigValue("ai_language");
  const spokenLanguages = useConfigValue("spoken_languages");
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const consentAutoSendChat = useConfigValue("consent_auto_send_chat");

  const start = useListener((state) => state.start);
  const { conn } = useSTTConnection();
  const runBatch = useRunBatch(sessionId);
  const { leftsidebar } = useShell();
  const setLeftSidebarExpanded = leftsidebar.setExpanded;

  const runBatchRef = useRef(runBatch);
  const canRunBatchRef = useRef(canRunBatchTranscription(conn));
  runBatchRef.current = runBatch;
  canRunBatchRef.current = canRunBatchTranscription(conn);

  const startListening = useCallback(async () => {
    let transcriptId: string | null = null;
    const startedAt = Date.now();
    const memoMd = session?.raw_md ?? "";
    const createdAt = new Date().toISOString();
    let lastTranscriptWrite = Promise.resolve();
    let transcriptWriteError: unknown;
    const trackTranscriptWrite = (write: Promise<void>) => {
      lastTranscriptWrite = write.catch((error) => {
        transcriptWriteError = error;
        console.error("[listener] failed to persist transcript", error);
      });
    };
    const keywords = await getSessionKeywords({
      sessionId,
      dictionaryTerms,
    });

    const onStopped: OnStoppedCallback = async (_sessionId, details) => {
      await lastTranscriptWrite;
      if (transcriptWriteError) return;

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

      const service = getEnhancerService();
      const shouldRegenerateExistingSummary =
        hadTranscriptBeforeStart &&
        (transcriptId !== null || postCaptureAction === "batch_then_enhance");
      if (shouldRegenerateExistingSummary) {
        await service?.resetEnhanceTasks(sessionId);
        service?.queueAutoEnhance(sessionId);
      } else {
        await service?.queueAutoEnhanceIfSummaryEmpty(sessionId);
      }

      await deleteProcessedAudioForRetention(audioRetention, sessionId);
    };

    const handlePersist: LiveTranscriptPersistCallback = (delta) => {
      if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
        return;
      }

      if (!transcriptId) {
        transcriptId = id();
        trackTranscriptWrite(
          createLiveTranscript(
            {
              id: transcriptId,
              sessionId,
              ownerUserId: session?.user_id ?? "",
              createdAt,
              startedAt,
              memo: memoMd,
              source: "live_capture",
              provider: conn?.provider,
              model: conn?.model,
            },
            delta,
          ),
        );
        return;
      }

      trackTranscriptWrite(
        applyLiveTranscriptDeltaToDatabase(transcriptId, delta),
      );
    };

    const languages = getTranscriptionLanguages(aiLanguage, spokenLanguages);
    const liveTranscriptionConfig = await getLiveTranscriptionConfig({
      provider: conn?.provider,
      model: conn?.model,
      languages,
    });

    const started = await start(
      {
        session_id: sessionId,
        languages: liveTranscriptionConfig.languages,
        onboarding: false,
        model: conn?.model ?? "",
        base_url: conn?.baseUrl ?? "",
        api_key: conn?.apiKey ?? "",
        keywords,
        transcription_mode: liveTranscriptionConfig.transcriptionMode,
        participant_human_ids: participantHumanIds,
        self_human_id: session?.user_id || null,
      },
      {
        handlePersist,
        onStopped,
      },
    );

    if (!started) {
      await lastTranscriptWrite;

      if (transcriptId) {
        await softDeleteTranscript(transcriptId);
      }
      return;
    }

    setLeftSidebarExpanded(false);

    if (consentAutoSendChat) {
      void sendConsentRequestToMeetingChat();
    }

    void analyticsCommands.event({
      event: "session_started",
      has_calendar_event: Boolean(
        getSessionEvent({ event_json: session?.event_json }),
      ),
      ...(conn
        ? {
            stt_provider: conn.provider,
            stt_model: conn.model,
          }
        : {}),
    });
  }, [
    aiLanguage,
    audioRetention,
    conn,
    dictionaryTerms,
    hadTranscriptBeforeStart,
    participantHumanIds,
    session,
    sessionId,
    start,
    spokenLanguages,
    setLeftSidebarExpanded,
    consentAutoSendChat,
  ]);

  return startListening;
}
