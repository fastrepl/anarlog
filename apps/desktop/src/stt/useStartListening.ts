import { useCallback, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as detectCommands } from "@hypr/plugin-detect";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { useListener } from "./contexts";
import { startMeetingChatCapture } from "./meeting-chat-capture";
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
import { catalogLocalSessionAudio } from "~/session/attachments";
import { enqueueSessionAudioOperation } from "~/session/audio-operations";
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

export const MEETING_DISCLOSURE_MESSAGE =
  "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";

const MEETING_DISCLOSURE_MAX_ATTEMPTS = 30;
const MEETING_DISCLOSURE_RETRY_INTERVAL_MS = 1_000;
const SLACK_BUNDLE_IDS = new Set([
  "com.slack.Slack",
  "com.tinyspeck.slackmacgap",
]);

type MeetingDisclosureOutcome =
  | { status: "sent" }
  | { status: "notSent"; reason: string }
  | { status: "cancelled" };

type MeetingDisclosureAttemptOutcome =
  | { status: "sent" }
  | { status: "notSent"; reason: unknown }
  | { status: "cancelled" };

type MeetingDisclosureTask = {
  cancelled: boolean;
  restartWhenSettled?: () => boolean;
  status: "sending" | "sent";
};

const meetingDisclosureTasks = new Map<string, MeetingDisclosureTask>();

function meetingDisclosureFailure(reason: unknown): MeetingDisclosureOutcome {
  const detail = reason instanceof Error ? reason.message : String(reason);
  console.warn("[listener] meeting disclosure was not sent", reason);
  sonnerToast.warning(
    "Recording started, but Anarlog could not post the meeting chat disclosure.",
    { id: "meeting-disclosure-send-failed" },
  );
  return { status: "notSent", reason: detail };
}

async function attemptMeetingRecordingDisclosure(
  isCancelled: () => boolean,
): Promise<MeetingDisclosureAttemptOutcome> {
  if (isCancelled()) {
    return { status: "cancelled" };
  }

  let micAppsResult: Awaited<
    ReturnType<typeof detectCommands.listMicUsingApplications>
  >;

  try {
    micAppsResult = await detectCommands.listMicUsingApplications();
  } catch (error) {
    return isCancelled()
      ? { status: "cancelled" }
      : { status: "notSent", reason: error };
  }

  if (isCancelled()) {
    return { status: "cancelled" };
  }

  if (micAppsResult.status === "error") {
    return { status: "notSent", reason: micAppsResult.error };
  }

  const micActiveBundleIds = [
    ...new Set(micAppsResult.data.map((app) => app.id.trim()).filter(Boolean)),
  ];
  if (!micActiveBundleIds.some((bundleId) => SLACK_BUNDLE_IDS.has(bundleId))) {
    return {
      status: "notSent",
      reason: "no mic-active Slack app was found",
    };
  }

  if (isCancelled()) {
    return { status: "cancelled" };
  }

  let result: Awaited<ReturnType<typeof detectCommands.sendMeetingChatMessage>>;

  try {
    result = await detectCommands.sendMeetingChatMessage(
      MEETING_DISCLOSURE_MESSAGE,
      micActiveBundleIds,
    );
  } catch (error) {
    return isCancelled()
      ? { status: "cancelled" }
      : { status: "notSent", reason: error };
  }

  if (result.status === "error") {
    return isCancelled()
      ? { status: "cancelled" }
      : { status: "notSent", reason: result.error };
  }

  if (result.data.sent) {
    return { status: "sent" };
  }

  if (isCancelled()) {
    return { status: "cancelled" };
  }

  return {
    status: "notSent",
    reason:
      result.data.warnings.join("; ") || "meeting chat mutation was rejected",
  };
}

export async function sendMeetingRecordingDisclosure({
  isCancelled = () => false,
  maxAttempts = MEETING_DISCLOSURE_MAX_ATTEMPTS,
  retryIntervalMs = MEETING_DISCLOSURE_RETRY_INTERVAL_MS,
}: {
  isCancelled?: () => boolean;
  maxAttempts?: number;
  retryIntervalMs?: number;
} = {}): Promise<MeetingDisclosureOutcome> {
  let lastFailureReason: unknown = "meeting chat disclosure was not sent";

  for (let attempt = 0; attempt < Math.max(1, maxAttempts); attempt += 1) {
    const outcome = await attemptMeetingRecordingDisclosure(isCancelled);
    if (outcome.status !== "notSent") {
      return outcome;
    }

    lastFailureReason = outcome.reason;
    if (attempt + 1 < Math.max(1, maxAttempts)) {
      await new Promise<void>((resolve) => {
        setTimeout(resolve, retryIntervalMs);
      });
      if (isCancelled()) {
        return { status: "cancelled" };
      }
    }
  }

  return meetingDisclosureFailure(lastFailureReason);
}

function startMeetingRecordingDisclosure(
  sessionId: string,
  isListening: () => boolean,
) {
  const existingTask = meetingDisclosureTasks.get(sessionId);
  if (existingTask) {
    if (existingTask.status === "sending" && existingTask.cancelled) {
      existingTask.restartWhenSettled = isListening;
    }
    return;
  }

  const task: MeetingDisclosureTask = {
    cancelled: false,
    status: "sending",
  };
  meetingDisclosureTasks.set(sessionId, task);

  void sendMeetingRecordingDisclosure({
    isCancelled: () => task.cancelled || !isListening(),
  }).then((outcome) => {
    if (meetingDisclosureTasks.get(sessionId) !== task) {
      return;
    }

    if (outcome.status === "sent") {
      task.status = "sent";
    } else {
      const restartWhenSettled = task.restartWhenSettled;
      meetingDisclosureTasks.delete(sessionId);
      if (restartWhenSettled?.()) {
        startMeetingRecordingDisclosure(sessionId, restartWhenSettled);
      }
    }
  });
}

function cancelMeetingRecordingDisclosure(sessionId: string) {
  const task = meetingDisclosureTasks.get(sessionId);
  if (!task || task.status === "sent") {
    return;
  }

  task.cancelled = true;
}

export function getPostCaptureAction(
  details: {
    audioPath: string | null;
    liveTranscriptionActive: boolean;
    needsBatchRepair: boolean;
  },
  canRunBatch: boolean,
) {
  if (details.liveTranscriptionActive && !details.needsBatchRepair) {
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
  const getSessionMode = useListener((state) => state.getSessionMode);

  const aiLanguage = useConfigValue("ai_language");
  const spokenLanguages = useConfigValue("spoken_languages");
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const meetingDisclosureAutoSendChat = useConfigValue(
    "consent_auto_send_chat",
  );

  const start = useListener((state) => state.start);
  const { conn } = useSTTConnection();
  const runBatch = useRunBatch(sessionId);
  const { leftsidebar } = useShell();
  const setLeftSidebarExpanded = leftsidebar.setExpanded;

  const runBatchRef = useRef(runBatch);
  const canRunBatchRef = useRef(canRunBatchTranscription(conn));
  const stopMeetingChatCaptureRef = useRef<(() => void) | null>(null);
  runBatchRef.current = runBatch;
  canRunBatchRef.current = canRunBatchTranscription(conn);

  const stopMeetingChatTasks = useCallback(() => {
    stopMeetingChatCaptureRef.current?.();
    stopMeetingChatCaptureRef.current = null;
  }, []);

  const startListening = useCallback(async () => {
    stopMeetingChatTasks();
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
      cancelMeetingRecordingDisclosure(sessionId);
      stopMeetingChatTasks();
      if (details.audioPath) {
        try {
          await enqueueSessionAudioOperation(sessionId, () =>
            catalogLocalSessionAudio(sessionId),
          );
        } catch (error) {
          console.error("[listener] failed to catalog recorded audio", error);
        }
      }
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

      if (postCaptureAction !== "none") {
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
      stopMeetingChatTasks();
      await lastTranscriptWrite;
      if (transcriptId) {
        await softDeleteTranscript(transcriptId);
      }
      return;
    }

    setLeftSidebarExpanded(false);

    stopMeetingChatCaptureRef.current = startMeetingChatCapture({
      sessionId,
      excludedTexts: [MEETING_DISCLOSURE_MESSAGE],
    });

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
    audioRetention,
    conn,
    dictionaryTerms,
    getSessionMode,
    hadTranscriptBeforeStart,
    participantHumanIds,
    session,
    sessionId,
    setLeftSidebarExpanded,
    meetingDisclosureAutoSendChat,
    spokenLanguages,
    start,
    stopMeetingChatTasks,
  ]);

  return startListening;
}
