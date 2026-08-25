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
import { useConfigValue } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";
import { useSessionParticipantHumanIds } from "~/stt/queries";

export {
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
  const {
    conn,
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
  const speakerDevice = useConfigValue("speaker_device");
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
    await lifecycle.ready;
    const { getSessionKeywords } = await import("./useKeywords");
    const keywords = await getSessionKeywords({
      sessionId,
      dictionaryTerms,
    });
    const languages = getTranscriptionLanguages(aiLanguage, spokenLanguages);
    const liveTranscriptionConfig = await getLiveTranscriptionConfig({
      provider: conn?.provider,
      model: conn?.model,
      languages,
    });
    if (!canStartLiveSession(sessionId)) {
      return;
    }
    try {
      await lifecycle.acquireCloudsyncLease();
    } catch (error) {
      console.error("[listener] failed to defer CloudSync for capture", error);
      trackAnalyticsEvent("session_start_failed", {
        failure_stage: "cloud_sync_deferral",
      });
      try {
        await lifecycle.releaseCloudsyncLease();
      } catch (cleanupError) {
        console.error(
          "[listener] failed to release capture CloudSync deferral",
          cleanupError,
        );
      }
      sonnerToast.error(
        "Anarlog could not safely start recording. Please try again.",
        { id: "capture-state-persist-failed" },
      );
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
      try {
        await lifecycle.releaseCloudsyncLease();
      } catch (releaseError) {
        console.error(
          "[listener] failed to release capture CloudSync deferral",
          releaseError,
        );
      }
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
          speaker_device: speakerDevice || null,
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
        await lifecycle.releaseCloudsyncLease();
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
        await lifecycle.releaseCloudsyncLease();
      }
      return;
    }

    if (!conn) {
      sonnerToast.warning("Live transcription is not configured", {
        id: "recording-without-transcription",
        duration: Infinity,
        description:
          "Audio is being saved. Choose a transcription provider to ensure this recording can be transcribed.",
        action: {
          label: "Configure",
          onClick: () => {
            openNew({
              type: "settings",
              state: { tab: "transcription" },
            });
          },
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
    speakerDevice,
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

  return startListening;
}
