import { useCallback, useRef } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { beginCloudsyncActivity } from "@anlg/plugin-db";
import { commands as detectCommands } from "@anlg/plugin-detect";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useListener } from "./contexts";
import { startMeetingChatCapture } from "./meeting-chat-capture";
import { persistTranscriptWrite } from "./persist-retry";
import { getSessionKeywords } from "./useKeywords";
import {
  canRunBatchTranscription,
  isStoppedTranscriptionError,
  useRunBatch,
} from "./useRunBatch";
import { useSTTConnection } from "./useSTTConnection";

import { requestMainAutoEnhance } from "~/ai/task-window-sync";
import { trackAnalyticsEvent } from "~/analytics";
import { useShell } from "~/contexts/shell";
import { releaseCloudsyncActivityEventually } from "~/db/cloudsync-activity";
import {
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
} from "~/services/audio-retention";
import { getEnhancerService } from "~/services/enhancer";
import { flushCanonicalSessionEditorChanges } from "~/session-sharing/editor-activity";
import {
  catalogLocalSessionAudio,
  markSessionAudioTranscriptionComplete,
} from "~/session/attachments";
import { enqueueSessionAudioOperation } from "~/session/audio-operations";
import { useSession, useSessionTranscriptExistence } from "~/session/queries";
import { getSessionEvent } from "~/session/utils";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import type {
  LiveTranscriptPersistCallback,
  OnStoppedCallback,
} from "~/store/zustand/listener/transcript";
import { useTabs } from "~/store/zustand/tabs";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";
import {
  type CaptureLifecycleMarker,
  clearCaptureLifecycleMarker,
  loadCaptureLifecycleMarker,
  saveCaptureLifecycleMarker,
} from "~/stt/capture-lifecycle-storage";
import { requestCaptureRecovery } from "~/stt/capture-recovery-requests";
import {
  applyLiveTranscriptDeltaToDatabase,
  createLiveTranscript,
  softDeleteTranscript,
  transcriptExists,
  useSessionParticipantHumanIds,
} from "~/stt/queries";

export const MEETING_DISCLOSURE_MESSAGE =
  "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";

const MEETING_DISCLOSURE_MAX_ATTEMPTS = 30;
const MEETING_DISCLOSURE_RETRY_INTERVAL_MS = 1_000;
const CLOUDSYNC_CAPTURE_ACTIVITY = "capture";
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

async function getAudioDurationMs(audioPath: string) {
  try {
    const metadataResult = await fsSyncCommands.audioSourceMetadata(audioPath);
    if (metadataResult.status === "error") {
      return null;
    }

    const durationMs = metadataResult.data.durationMs;
    return typeof durationMs === "number" && Number.isFinite(durationMs)
      ? Math.max(0, durationMs)
      : null;
  } catch {
    return null;
  }
}

async function getExistingAudioDurationMs(sessionId: string) {
  try {
    const pathResult = await fsSyncCommands.audioPath(sessionId);
    if (pathResult.status === "error") {
      return 0;
    }

    return (await getAudioDurationMs(pathResult.data)) ?? 0;
  } catch {
    return 0;
  }
}

async function requestCaptureRecoverySafely(sessionId: string) {
  try {
    await requestCaptureRecovery(sessionId);
  } catch (error) {
    console.error("[listener] failed to request capture recovery", error);
  }
}

type PostCaptureDetails = {
  audioPath: string | null;
  liveTranscriptionActive: boolean;
  needsBatchRepair: boolean;
  transcriptWriteFailed?: boolean;
};

export type PostCaptureRepairReason =
  | "live_transcription_unavailable"
  | "live_stream_incomplete"
  | "transcript_persistence_failed";

export function getPostCaptureRepairReasons(
  details: PostCaptureDetails,
): PostCaptureRepairReason[] {
  const reasons: PostCaptureRepairReason[] = [];
  if (!details.liveTranscriptionActive) {
    reasons.push("live_transcription_unavailable");
  }
  if (details.needsBatchRepair) {
    reasons.push("live_stream_incomplete");
  }
  if (details.transcriptWriteFailed) {
    reasons.push("transcript_persistence_failed");
  }
  return reasons;
}

export function getPostCaptureAction(
  details: PostCaptureDetails,
  canRunBatch: boolean,
) {
  if (
    details.liveTranscriptionActive &&
    !details.needsBatchRepair &&
    !details.transcriptWriteFailed
  ) {
    return "enhance_only" as const;
  }

  if (!!details.audioPath && canRunBatch) {
    return "batch_then_enhance" as const;
  }

  return "none" as const;
}

function useCaptureLifecycle(sessionId: string) {
  const session = useSession(sessionId);
  const transcriptExistence = useSessionTranscriptExistence(sessionId);
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const { conn } = useSTTConnection();
  const runBatch = useRunBatch(sessionId);

  const runBatchRef = useRef(runBatch);
  const canRunBatchRef = useRef(canRunBatchTranscription(conn));
  const stopMeetingChatCaptureRef = useRef<(() => Promise<void>) | null>(null);
  runBatchRef.current = runBatch;
  canRunBatchRef.current = canRunBatchTranscription(conn);

  const stopMeetingChatTasks = useCallback(async () => {
    const stop = stopMeetingChatCaptureRef.current;
    if (!stop) {
      return;
    }
    await stop();
    if (stopMeetingChatCaptureRef.current === stop) {
      stopMeetingChatCaptureRef.current = null;
    }
  }, []);
  const setStopMeetingChatCapture = useCallback(
    (stop: (() => Promise<void>) | null) => {
      stopMeetingChatCaptureRef.current = stop;
    },
    [],
  );

  const createCaptureLifecycle = useCallback(
    (recoveredMarker?: CaptureLifecycleMarker) => {
      const transcriptId = recoveredMarker?.transcriptId ?? id();
      let transcriptCreated: boolean | null = recoveredMarker ? null : false;
      let transcriptTouched = false;
      const startedAt = recoveredMarker?.startedAt ?? Date.now();
      const memoMd = recoveredMarker?.memo ?? session?.raw_md ?? "";
      const createdAt = recoveredMarker?.createdAt ?? new Date().toISOString();
      const preserveExistingTranscript =
        recoveredMarker?.preserveExistingTranscript ??
        transcriptExistence !== false;
      const ownerUserId =
        recoveredMarker?.ownerUserId ?? session?.user_id ?? "";
      const provider = recoveredMarker?.provider ?? conn?.provider;
      const model = recoveredMarker?.model ?? conn?.model;
      const cloudsyncLeaseKey = `${sessionId}:${transcriptId}`;
      let pendingSummaryMode = recoveredMarker?.summaryMode;
      let completionTracked = false;
      let capturePhase =
        recoveredMarker?.phase ??
        (recoveredMarker?.summaryMode ? "finalizing" : "capturing");
      const existingAudioDurationPromise = recoveredMarker
        ? Promise.resolve(recoveredMarker.audioOffsetMs)
        : preserveExistingTranscript
          ? getExistingAudioDurationMs(sessionId)
          : Promise.resolve(0);
      let lastTranscriptWrite = Promise.resolve();
      let transcriptWriteError: unknown;
      let cloudsyncLeaseActive = false;
      let cloudsyncLeaseAcquire: Promise<void> | null = null;
      let cloudsyncLeaseRelease: Promise<void> | null = null;
      let recoveryPending = Boolean(recoveredMarker);
      let recoveryStateCleared = false;
      const handoffCloudsyncLease = () => {
        cloudsyncLeaseActive = false;
        cloudsyncLeaseAcquire = null;
        cloudsyncLeaseRelease = null;
      };
      const releaseCloudsyncLease = () => {
        if (cloudsyncLeaseRelease) {
          return cloudsyncLeaseRelease;
        }
        if (!cloudsyncLeaseActive) {
          return Promise.resolve();
        }
        cloudsyncLeaseRelease = releaseCloudsyncActivityEventually(
          CLOUDSYNC_CAPTURE_ACTIVITY,
          cloudsyncLeaseKey,
        ).then(
          () => {
            cloudsyncLeaseActive = false;
            cloudsyncLeaseAcquire = null;
            cloudsyncLeaseRelease = null;
          },
          (error) => {
            cloudsyncLeaseRelease = null;
            console.warn(
              "[listener] failed to release capture CloudSync deferral",
              error,
            );
            throw error;
          },
        );
        return cloudsyncLeaseRelease;
      };
      const acquireCloudsyncLease = async () => {
        if (cloudsyncLeaseRelease) {
          await cloudsyncLeaseRelease;
        }
        cloudsyncLeaseActive = true;
        cloudsyncLeaseAcquire ??= beginCloudsyncActivity(
          CLOUDSYNC_CAPTURE_ACTIVITY,
          cloudsyncLeaseKey,
        );
        const acquisition = cloudsyncLeaseAcquire;
        try {
          await acquisition;
        } catch (error) {
          if (cloudsyncLeaseAcquire === acquisition) {
            cloudsyncLeaseAcquire = null;
            await releaseCloudsyncLease();
          }
          throw error;
        }
      };
      const trackTranscriptWrite = (write: () => Promise<void>) => {
        lastTranscriptWrite = lastTranscriptWrite
          .then(() => persistTranscriptWrite(write))
          .catch((error) => {
            transcriptWriteError = error;
            console.error("[listener] failed to persist transcript", error);
          });
      };
      const marker = async (): Promise<CaptureLifecycleMarker> => ({
        version: 1,
        phase: capturePhase,
        sessionId,
        transcriptId,
        startedAt,
        createdAt,
        audioOffsetMs: await existingAudioDurationPromise,
        preserveExistingTranscript,
        ownerUserId,
        memo: memoMd,
        ...(provider ? { provider } : {}),
        ...(model ? { model } : {}),
        ...(pendingSummaryMode ? { summaryMode: pendingSummaryMode } : {}),
      });
      const finalizeStoppedInner = async (
        details: Parameters<OnStoppedCallback>[1],
        requestRecoveryOnFailure: boolean,
      ) => {
        const requestRecovery = async () => {
          recoveryPending = true;
          if (requestRecoveryOnFailure) {
            await requestCaptureRecoverySafely(sessionId);
          }
        };
        const finishCaptureSyncDeferral = async (): Promise<boolean> => {
          if (capturePhase !== "finalizing") {
            capturePhase = "finalizing";
            try {
              await persistTranscriptWrite(async () => {
                await saveCaptureLifecycleMarker(await marker());
              });
            } catch (error) {
              console.error(
                "[listener] failed to finalize capture recovery state",
                error,
              );
              await requestRecovery();
              return false;
            }
          }
          return true;
        };
        cancelMeetingRecordingDisclosure(sessionId);
        await stopMeetingChatTasks();
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
        transcriptCreated ??= await transcriptExists(transcriptId);

        const postCaptureAction = pendingSummaryMode
          ? ("enhance_only" as const)
          : getPostCaptureAction(
              {
                ...details,
                transcriptWriteFailed: Boolean(transcriptWriteError),
              },
              canRunBatchRef.current,
            );
        const repairReasons = pendingSummaryMode
          ? []
          : getPostCaptureRepairReasons({
              ...details,
              transcriptWriteFailed: Boolean(transcriptWriteError),
            });

        let batchCompleted = false;
        if (postCaptureAction === "batch_then_enhance") {
          console.info("[listener] starting post-stop transcript repair", {
            sessionId,
            reasons: repairReasons,
          });
          try {
            const existingAudioDurationMs = await existingAudioDurationPromise;
            const finalAudioDurationMs = preserveExistingTranscript
              ? await getAudioDurationMs(details.audioPath!)
              : null;
            const audioOffsetMs =
              existingAudioDurationMs > 0 &&
              finalAudioDurationMs !== null &&
              finalAudioDurationMs + 1_000 >= existingAudioDurationMs
                ? Math.min(existingAudioDurationMs, finalAudioDurationMs)
                : 0;
            await runBatchRef.current(details.audioPath!, {
              deferAudioFinalization: true,
              promotion: preserveExistingTranscript
                ? {
                    scope: "current_capture",
                    audioOffsetMs,
                    ...(transcriptCreated
                      ? { replaceTranscriptId: transcriptId }
                      : {}),
                    startedAt,
                  }
                : { scope: "whole_session" },
            });
            batchCompleted = true;
            console.info("[listener] completed post-stop transcript repair", {
              sessionId,
              reasons: repairReasons,
            });
          } catch (error) {
            if (isStoppedTranscriptionError(error)) {
              await requestRecovery();
              return;
            }
            console.error("[listener] post-stop transcript repair failed", {
              sessionId,
              reasons: repairReasons,
              error,
            });
            trackAnalyticsEvent("transcription_failed", {
              mode: "post_capture",
              failure_stage: "batch_repair",
            });
            if (transcriptWriteError || !details.liveTranscriptionActive) {
              sonnerToast.error(
                "Anarlog could not finish saving the transcript. The recording was kept so you can try again.",
                { id: "post-capture-transcript-incomplete" },
              );
            } else {
              sonnerToast.error(
                "Post-meeting transcription failed. The recording was kept so you can try again.",
                { id: "post-capture-batch-failed" },
              );
            }
            await requestRecovery();
            return;
          }
        }

        if (
          transcriptWriteError &&
          postCaptureAction !== "batch_then_enhance"
        ) {
          sonnerToast.error(
            details.audioPath
              ? "Anarlog could not finish saving the transcript. The recording was kept so you can try again."
              : "Anarlog could not save part of the live transcript.",
            {
              id: details.audioPath
                ? "post-capture-transcript-incomplete"
                : "live-transcript-persist-failed",
            },
          );
        }

        const emptyFreshCapture =
          !recoveredMarker &&
          !details.audioPath &&
          !transcriptTouched &&
          !transcriptWriteError;
        const transcriptIsComplete =
          Boolean(pendingSummaryMode) ||
          batchCompleted ||
          postCaptureAction === "enhance_only" ||
          emptyFreshCapture;
        if (!transcriptIsComplete) {
          trackAnalyticsEvent("transcription_failed", {
            mode: "live",
            failure_stage: "persist",
          });
          await requestRecovery();
          return;
        }
        if (!(await finishCaptureSyncDeferral())) {
          return;
        }

        try {
          await flushCanonicalSessionEditorChanges(sessionId);
        } catch (error) {
          console.error(
            "[listener] failed to flush session notes before completing capture",
            error,
          );
          await requestRecovery();
          return;
        }

        const hasTranscriptEvidence =
          Boolean(pendingSummaryMode) ||
          preserveExistingTranscript ||
          transcriptTouched ||
          batchCompleted;
        const shouldEnhance =
          hasTranscriptEvidence &&
          (transcriptIsComplete ||
            (postCaptureAction === "none" &&
              preserveExistingTranscript &&
              !transcriptWriteError));

        let summaryScheduled = true;
        if (shouldEnhance) {
          const summaryMode =
            pendingSummaryMode ??
            (preserveExistingTranscript && (transcriptTouched || batchCompleted)
              ? "regenerate"
              : "if_empty");
          if (!pendingSummaryMode) {
            pendingSummaryMode = summaryMode;
            try {
              await persistTranscriptWrite(async () => {
                await saveCaptureLifecycleMarker(await marker());
              });
            } catch (error) {
              pendingSummaryMode = undefined;
              console.error(
                "[listener] failed to persist summary recovery state",
                error,
              );
              sonnerToast.error(
                "The transcript was saved, but Anarlog could not start the summary. Try generating it again.",
                { id: "post-capture-summary-failed" },
              );
              await requestRecovery();
              return;
            }
          }
          try {
            const service = getEnhancerService();
            if (!service) {
              await requestMainAutoEnhance(sessionId, summaryMode);
            } else {
              await service.requestAutoEnhance(sessionId, summaryMode);
            }
          } catch (error) {
            summaryScheduled = false;
            console.error("[listener] failed to schedule summary", error);
            sonnerToast.error(
              "The transcript was saved, but Anarlog could not start the summary. Try generating it again.",
              { id: "post-capture-summary-failed" },
            );
          }
        }

        const recoveryComplete =
          (!details.audioPath && !transcriptWriteError) ||
          (transcriptIsComplete && summaryScheduled);
        if (!recoveryComplete) {
          await requestRecovery();
          return;
        }

        try {
          if (details.audioPath && transcriptIsComplete) {
            await persistTranscriptWrite(() =>
              markSessionAudioTranscriptionComplete(sessionId),
            );
          }
          await clearCaptureLifecycleMarker(sessionId, transcriptId);
          recoveryPending = false;
          recoveryStateCleared = true;
          if (hasTranscriptEvidence && !batchCompleted) {
            trackAnalyticsEvent("transcription_completed", {
              mode: "live",
            });
          }
        } catch (error) {
          await requestRecovery();
          throw error;
        }

        // A failed batch repair — or a live transcript that never fully
        // persisted — keeps the recording around as the only source for a
        // later repair, regardless of the retention policy.
        if (
          (postCaptureAction !== "batch_then_enhance" || batchCompleted) &&
          !transcriptWriteError
        ) {
          await deleteProcessedAudioForRetention(audioRetention, sessionId);
        }
      };
      const finalizeStopped = async (
        details: Parameters<OnStoppedCallback>[1],
        requestRecoveryOnFailure: boolean,
      ) => {
        try {
          await finalizeStoppedInner(details, requestRecoveryOnFailure);
        } catch (error) {
          if (!recoveryStateCleared && !recoveryPending) {
            recoveryPending = true;
            if (requestRecoveryOnFailure) {
              await requestCaptureRecoverySafely(sessionId);
            }
          }
          throw error;
        } finally {
          if (recoveryPending) {
            if (requestRecoveryOnFailure) {
              handoffCloudsyncLease();
            }
          } else {
            await releaseCloudsyncLease();
          }
        }
      };
      const trackSessionCompletion = (
        details: Parameters<OnStoppedCallback>[1],
        completionReason: "capture_stopped" | "recovered_capture_stopped",
      ) => {
        if (!completionTracked) {
          completionTracked = true;
          trackAnalyticsEvent("session_completed", {
            duration_seconds: Math.max(
              0,
              Math.round((Date.now() - startedAt) / 1_000),
            ),
            transcription_requested:
              details.liveTranscriptionActive || canRunBatchRef.current,
            completion_reason: completionReason,
          });
        }
      };
      const onStopped: OnStoppedCallback = (_sessionId, details) => {
        trackSessionCompletion(
          details,
          recoveredMarker ? "recovered_capture_stopped" : "capture_stopped",
        );
        recoveryPending = false;
        return finalizeStopped(details, true);
      };
      const recoverStopped: OnStoppedCallback = (_sessionId, details) => {
        trackSessionCompletion(details, "recovered_capture_stopped");
        return finalizeStopped(details, false);
      };

      const handlePersist: LiveTranscriptPersistCallback = (delta) => {
        if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
          return;
        }

        transcriptTouched = true;
        trackTranscriptWrite(async () => {
          transcriptCreated ??= await transcriptExists(transcriptId);
          if (!transcriptCreated) {
            await createLiveTranscript(
              {
                id: transcriptId,
                sessionId,
                ownerUserId,
                createdAt,
                startedAt,
                memo: memoMd,
                source: "live_capture",
                provider,
                model,
              },
              delta,
            );
            transcriptCreated = true;
            return;
          }

          await applyLiveTranscriptDeltaToDatabase(transcriptId, delta);
        });
      };

      return {
        acquireCloudsyncLease,
        handlePersist,
        onStopped,
        recoverStopped,
        ready: existingAudioDurationPromise.then(() => undefined),
        persistMarker: async () => {
          await persistTranscriptWrite(async () => {
            await saveCaptureLifecycleMarker(await marker());
          });
        },
        cleanupFailedStart: async () => {
          await lastTranscriptWrite;
          await clearCaptureLifecycleMarker(sessionId, transcriptId);
          if (transcriptCreated) {
            await softDeleteTranscript(transcriptId);
          }
        },
        releaseCloudsyncLease,
      };
    },
    [
      audioRetention,
      conn?.model,
      conn?.provider,
      session?.raw_md,
      session?.user_id,
      sessionId,
      stopMeetingChatTasks,
      transcriptExistence,
    ],
  );

  return {
    conn,
    createCaptureLifecycle,
    session,
    setStopMeetingChatCapture,
    stopMeetingChatTasks,
  };
}

export function useResumeListeningLifecycle(sessionId: string) {
  const attachLiveSession = useListener((state) => state.attachLiveSession);
  const beginCaptureRecoveryFinalization = useListener(
    (state) => state.beginCaptureRecoveryFinalization,
  );
  const finishCaptureRecoveryFinalization = useListener(
    (state) => state.finishCaptureRecoveryFinalization,
  );
  const { createCaptureLifecycle } = useCaptureLifecycle(sessionId);
  const createCaptureLifecycleRef = useRef(createCaptureLifecycle);
  createCaptureLifecycleRef.current = createCaptureLifecycle;
  const recoveryAttemptRef = useRef<{
    sessionId: string;
    lifecycleState: Promise<{
      lifecycle: ReturnType<typeof createCaptureLifecycle>;
      ensureMarker: () => Promise<void>;
      hasMarker: () => boolean;
    }>;
    stoppedProcessingRef: { current: Promise<void> | null };
  } | null>(null);
  const ownsRecoveryFinalizationRef = useRef(false);

  return useCallback(async () => {
    let attempt = recoveryAttemptRef.current;
    if (!attempt || attempt.sessionId !== sessionId) {
      const stoppedProcessingRef = {
        current: null as Promise<void> | null,
      };
      const lifecycleState = loadCaptureLifecycleMarker(sessionId).then(
        (recoveredMarker) => {
          let markerInstalled = Boolean(recoveredMarker);
          let markerWrite: Promise<void> | null = null;
          const lifecycle = createCaptureLifecycleRef.current(
            recoveredMarker ?? undefined,
          );
          return {
            lifecycle,
            hasMarker: () => markerInstalled,
            ensureMarker: () => {
              if (markerInstalled) {
                return Promise.resolve();
              }
              markerWrite ??= lifecycle.persistMarker().then(
                () => {
                  markerInstalled = true;
                },
                (error) => {
                  markerWrite = null;
                  throw error;
                },
              );
              return markerWrite;
            },
          };
        },
      );
      attempt = { sessionId, lifecycleState, stoppedProcessingRef };
      recoveryAttemptRef.current = attempt;
      void lifecycleState.catch((error) => {
        console.error(
          "[listener] failed to load capture recovery state",
          error,
        );
      });
    }

    const { lifecycleState, stoppedProcessingRef } = attempt;
    let state: Awaited<typeof lifecycleState>;
    try {
      state = await lifecycleState;
      await state.lifecycle.acquireCloudsyncLease();
    } catch (error) {
      console.error(
        "[listener] failed to prepare capture recovery state",
        error,
      );
      return "error" as const;
    }

    let result: Awaited<ReturnType<typeof attachLiveSession>>;
    try {
      result = await attachLiveSession(sessionId, {
        handlePersist: (delta) => {
          void state.lifecycle
            .acquireCloudsyncLease()
            .then(() => state.lifecycle.handlePersist(delta))
            .catch((error) => {
              console.error(
                "[listener] failed to recover transcript persistence",
                error,
              );
            });
        },
        onStopped: (stoppedSessionId, details) => {
          const processing = state.lifecycle.acquireCloudsyncLease().then(() =>
            state.lifecycle.onStopped(stoppedSessionId, {
              ...details,
              needsBatchRepair: true,
            }),
          );
          stoppedProcessingRef.current = processing;
          return processing;
        },
      });
    } catch (error) {
      console.error("[listener] failed to attach capture recovery", error);
      return "error" as const;
    }

    if (result === "attached") {
      try {
        await state.ensureMarker();
      } catch (error) {
        console.error(
          "[listener] failed to prepare capture recovery state",
          error,
        );
        return "error" as const;
      }
      return result;
    }
    if (result === "error") {
      const stoppedProcessing = stoppedProcessingRef.current;
      if (stoppedProcessing) {
        try {
          await stoppedProcessing;
        } catch (error) {
          console.error("[listener] failed to recover stopped capture", error);
          if (stoppedProcessingRef.current === stoppedProcessing) {
            stoppedProcessingRef.current = null;
          }
        }
      }
      return "error" as const;
    }
    if (stoppedProcessingRef.current) {
      const stoppedProcessing = stoppedProcessingRef.current;
      try {
        await stoppedProcessing;
      } catch (error) {
        console.error("[listener] failed to recover stopped capture", error);
        if (stoppedProcessingRef.current === stoppedProcessing) {
          stoppedProcessingRef.current = null;
        }
        return "error" as const;
      }
      if (await loadCaptureLifecycleMarker(sessionId)) {
        if (stoppedProcessingRef.current === stoppedProcessing) {
          stoppedProcessingRef.current = null;
        }
      } else {
        if (ownsRecoveryFinalizationRef.current) {
          finishCaptureRecoveryFinalization(sessionId);
          ownsRecoveryFinalizationRef.current = false;
        }
        return "inactive" as const;
      }
    }

    if (!state.hasMarker() || !(await loadCaptureLifecycleMarker(sessionId))) {
      if (ownsRecoveryFinalizationRef.current) {
        finishCaptureRecoveryFinalization(sessionId);
        ownsRecoveryFinalizationRef.current = false;
      }
      await state.lifecycle.releaseCloudsyncLease();
      return "inactive" as const;
    }

    if (!ownsRecoveryFinalizationRef.current) {
      if (!beginCaptureRecoveryFinalization(sessionId)) {
        return "error" as const;
      }
      ownsRecoveryFinalizationRef.current = true;
    }

    let audioPath: string | null = null;
    let durationSeconds = 0;
    try {
      const pathResult = await fsSyncCommands.audioPath(sessionId);
      if (pathResult.status === "ok") {
        audioPath = pathResult.data;
        durationSeconds =
          ((await getAudioDurationMs(pathResult.data)) ?? 0) / 1_000;
      } else if (pathResult.error !== "audio_path_not_found") {
        throw new Error(pathResult.error);
      }
      await state.lifecycle.recoverStopped(sessionId, {
        durationSeconds,
        audioPath,
        requestedLiveTranscription: true,
        liveTranscriptionActive: false,
        needsBatchRepair: true,
      });
    } catch (error) {
      console.error("[listener] failed to recover stopped capture", error);
      return "error" as const;
    }

    if (await loadCaptureLifecycleMarker(sessionId)) {
      return "error" as const;
    }
    finishCaptureRecoveryFinalization(sessionId);
    ownsRecoveryFinalizationRef.current = false;
    return "inactive" as const;
  }, [
    attachLiveSession,
    beginCaptureRecoveryFinalization,
    finishCaptureRecoveryFinalization,
    sessionId,
  ]);
}

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
  const meetingDisclosureAutoSendChat = useConfigValue(
    "consent_auto_send_chat",
  );

  const start = useListener((state) => state.start);
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
    stopMeetingChatTasks,
  ]);

  return startListening;
}
