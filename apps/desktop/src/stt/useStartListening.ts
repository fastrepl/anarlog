import { useCallback } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useCaptureLifecycle } from "./capture-lifecycle";
import { useListener } from "./contexts";
import { startMeetingChatCapture } from "./meeting-chat-capture";
import {
  MEETING_DISCLOSURE_MESSAGE,
  startMeetingRecordingDisclosure,
} from "./meeting-disclosure";

import { trackAnalyticsEvent } from "~/analytics";
import { useShell } from "~/contexts/shell";
import { getSessionEvent } from "~/session/utils";
import { getBaseLanguageDisplayName } from "~/settings/general/language";
import { useConfigValue } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";
import { useSessionParticipantHumanIds } from "~/stt/queries";

export {
  CLOUDSYNC_CAPTURE_LEASE_ATTEMPTS,
  getPostCaptureAction,
  getPostCaptureRepairReasons,
  type PostCaptureRepairReason,
} from "./capture-lifecycle";
export {
  MEETING_DISCLOSURE_MESSAGE,
  sendMeetingRecordingDisclosure,
} from "./meeting-disclosure";
export { useResumeListeningLifecycle } from "./resume-listening";

export function useStartListening(sessionId: string) {
  return useStartListeningState(sessionId).startListening;
}

export function useStartListeningState(sessionId: string) {
  const {
    conn,
    connectionReady,
    createCaptureLifecycle,
    session,
    setStopMeetingChatCapture,
    stopMeetingChatTasks,
  } = useCaptureLifecycle(sessionId);
  const participantHumanIds = useSessionParticipantHumanIds(sessionId);
  const getSessionMode = useListener((state) => state.getSessionMode);
  const canStartLiveSession = useListener((state) => state.canStartLiveSession);

  const aiLanguage = useConfigValue("ai_language");
  const spokenLanguages = useConfigValue("spoken_languages");
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");
  const microphoneDevice = useConfigValue("microphone_device");
  const meetingDisclosureAutoSendChat = useConfigValue(
    "consent_auto_send_chat",
  );

  const start = useListener((state) => state.start);
  const stop = useListener((state) => state.stop);
  const { leftsidebar } = useShell();
  const setLeftSidebarExpanded = leftsidebar.setExpanded;
  const openNew = useTabs((state) => state.openNew);

  const startListening = useCallback(async () => {
    if (!canStartLiveSession(sessionId)) {
      return;
    }
    await stopMeetingChatTasks();
    const lifecycle = createCaptureLifecycle();
    // A fresh note or a just-focused window starts listening right as a sync
    // round begins; waiting for that round to yield made the start feel slow
    // and sometimes refused to record at all.
    void lifecycle.deferCloudsync();
    const releaseCloudsyncDeferral = async () => {
      try {
        await lifecycle.releaseCloudsyncLease();
      } catch (error) {
        console.error(
          "[listener] failed to release capture CloudSync deferral",
          error,
        );
      }
    };
    const [keywords, liveTranscriptionConfig] = await Promise.all([
      import("./useKeywords").then(({ getSessionKeywords }) =>
        getSessionKeywords({ sessionId, dictionaryTerms }),
      ),
      getLiveTranscriptionConfig({
        provider: conn?.provider,
        model: conn?.model,
        languages: getTranscriptionLanguages(aiLanguage, spokenLanguages),
      }),
      lifecycle.ready,
    ]);
    if (!canStartLiveSession(sessionId)) {
      await releaseCloudsyncDeferral();
      return;
    }

    try {
      await lifecycle.persistMarker();
    } catch (error) {
      console.error(
        "[listener] failed to prepare durable capture state",
        error,
      );
      trackAnalyticsEvent("session_start_failed", {
        failure_stage: "recovery_marker",
      });
      try {
        await lifecycle.cleanupFailedStart();
      } catch (cleanupError) {
        console.error(
          "[listener] failed to clean up capture state",
          cleanupError,
        );
      }
      await releaseCloudsyncDeferral();
      sonnerToast.error(
        "Anarlog could not safely start recording. Please try again.",
        { id: "capture-state-persist-failed" },
      );
      return;
    }

    let started = false;
    try {
      started = await start(
        {
          session_id: sessionId,
          languages: liveTranscriptionConfig.languages,
          onboarding: false,
          model: conn?.model ?? "",
          base_url: conn?.baseUrl ?? "",
          api_key: conn?.apiKey ?? "",
          keywords,
          mic_device: microphoneDevice || null,
          transcription_mode: liveTranscriptionConfig.transcriptionMode,
          participant_human_ids: participantHumanIds,
          self_human_id: session?.user_id || null,
        },
        {
          handlePersist: lifecycle.handlePersist,
          onStopped: lifecycle.onStopped,
        },
      );
    } catch (error) {
      console.error("[listener] failed to start recording", error);
      trackAnalyticsEvent("session_start_failed", {
        failure_stage: "capture_start",
      });
      try {
        await lifecycle.cleanupFailedStart();
      } catch (cleanupError) {
        console.error(
          "[listener] failed to clean up capture state",
          cleanupError,
        );
      } finally {
        await releaseCloudsyncDeferral();
      }
      sonnerToast.error(
        "Anarlog could not safely start recording. Please try again.",
        { id: "capture-state-persist-failed" },
      );
      return;
    }

    if (!started) {
      trackAnalyticsEvent("session_start_failed", {
        failure_stage: "capture_rejected",
      });
      await stopMeetingChatTasks();
      try {
        await lifecycle.cleanupFailedStart();
      } catch (error) {
        console.error("[listener] failed to clean up capture state", error);
        sonnerToast.error(
          "Anarlog could not safely start recording. Please try again.",
          { id: "capture-state-persist-failed" },
        );
      } finally {
        await releaseCloudsyncDeferral();
      }
      return;
    }

    const openTranscriptionSettings = () => {
      openNew({
        type: "settings",
        state: { tab: "transcription" },
      });
    };

    const primaryLanguage = liveTranscriptionConfig.languages[0];
    const omittedLanguages = liveTranscriptionConfig.omittedLanguages ?? [];
    if (conn && primaryLanguage && omittedLanguages.length > 0) {
      const primaryLanguageName = getBaseLanguageDisplayName(primaryLanguage);
      const omittedLanguageNames = omittedLanguages
        .map((language) => getBaseLanguageDisplayName(language))
        .join(", ");

      sonnerToast.warning(
        `Live transcription is using ${primaryLanguageName}`,
        {
          id: "recording-with-limited-transcription-languages",
          duration: Infinity,
          description: `Live transcription won't include ${omittedLanguageNames}. Audio is still being saved.`,
          action: {
            label: "Change",
            onClick: openTranscriptionSettings,
          },
        },
      );
    } else if (!conn) {
      sonnerToast.warning("Live transcription is not configured", {
        id: "recording-without-transcription",
        duration: Infinity,
        description:
          "Audio is being saved. Choose a transcription provider to ensure this recording can be transcribed.",
        action: {
          label: "Configure",
          onClick: openTranscriptionSettings,
        },
      });
    }

    setLeftSidebarExpanded(false);

    setStopMeetingChatCapture(
      startMeetingChatCapture({
        sessionId,
        excludedTexts: [MEETING_DISCLOSURE_MESSAGE],
        onParticipantDeclined: () => {
          sonnerToast.warning(
            "A participant declined recording. Anarlog stopped listening.",
            { id: "meeting-consent-declined", duration: Infinity },
          );
          stop();
        },
      }),
    );

    if (meetingDisclosureAutoSendChat) {
      startMeetingRecordingDisclosure(
        sessionId,
        () => getSessionMode(sessionId) === "active",
      );
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
    canStartLiveSession,
    conn,
    createCaptureLifecycle,
    dictionaryTerms,
    getSessionMode,
    microphoneDevice,
    openNew,
    participantHumanIds,
    session,
    sessionId,
    setStopMeetingChatCapture,
    setLeftSidebarExpanded,
    meetingDisclosureAutoSendChat,
    spokenLanguages,
    start,
    stop,
    stopMeetingChatTasks,
  ]);

  return { connectionReady, startListening };
}
