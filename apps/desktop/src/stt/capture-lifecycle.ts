import { useCallback, useRef } from "react";

import { beginCloudsyncActivity } from "@anlg/plugin-db";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import {
  type RecoveryAudioChunk,
  commands as transcriptionCommands,
  events as transcriptionEvents,
  type LiveTranscriptTarget,
} from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";

import {
  chunkInterval,
  createCaptureAudioRecovery,
} from "./capture-audio-recovery";
import {
  saveIncompleteCapture,
  clearIncompleteCapture,
} from "./capture-result";
import { useListener } from "./contexts";
import { discardEmptyAutomaticCapture } from "./empty-automatic-capture";
import { cancelMeetingRecordingDisclosure } from "./meeting-disclosure";
import { createNativeTranscriptPersistence } from "./native-transcript-persistence";
import { persistTranscriptWrite } from "./persist-retry";
import { consumePrimaryDeviceYield } from "./primary-device";
import {
  canRunBatchTranscription,
  isStoppedTranscriptionError,
  isTerminalTranscriptionError,
  reconcileRefinedSpeakerClusters,
  useRunBatch,
} from "./useRunBatch";
import { useSTTConnection } from "./useSTTConnection";

import { requestMainAutoEnhance } from "~/ai/task-window-sync";
import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { releaseCloudsyncActivityEventually } from "~/db/cloudsync-activity";
import {
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
} from "~/services/audio-retention";
import { getEnhancerService } from "~/services/enhancer";
import { maybeExtractVoiceprintCandidates } from "~/services/voiceprint";
import { flushCanonicalSessionEditorChanges } from "~/session-sharing/editor-activity";
import {
  catalogLocalSessionAudio,
  markSessionAudioTranscriptionComplete,
} from "~/session/attachments";
import { enqueueSessionAudioOperation } from "~/session/audio-operations";
import {
  isSessionDeleted,
  useSession,
  useSessionTranscriptExistence,
} from "~/session/queries";
import { requestAppAttention } from "~/shared/app-attention";
import { playCompletionSound } from "~/shared/completion-sound";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import { listenerStore } from "~/store/zustand/listener/instance";
import type {
  LiveTranscriptPersistCallback,
  OnStoppedCallback,
} from "~/store/zustand/listener/transcript";
import {
  isRealtimeLocalModel,
  requiresRetainedBatchAudio,
} from "~/stt/capabilities";
import {
  type CaptureLifecycleMarker,
  clearCaptureLifecycleMarker,
  type InheritedCapture,
  saveCaptureLifecycleMarker,
} from "~/stt/capture-lifecycle-storage";
import { requestCaptureRecovery } from "~/stt/capture-recovery-requests";
import {
  appendRecoveredTranscriptWords,
  getTranscriptRecord,
  createLiveTranscript,
  flushLiveTranscriptDeltasToDatabase,
  softDeleteTranscript,
  transcriptExists,
  useSessionParticipantHumanIds,
} from "~/stt/queries";
import { waitForSessionSearchIndex } from "~/stt/search-index-consistency";

const CLOUDSYNC_CAPTURE_ACTIVITY = "capture";
export const CLOUDSYNC_CAPTURE_LEASE_ATTEMPTS = 3;

export async function getAudioDurationMs(audioPath: string) {
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
  refineSpeakerDiarization?: boolean;
  transcriptWriteFailed?: boolean;
};

export type PostCaptureRepairReason =
  | "live_transcription_unavailable"
  | "live_stream_incomplete"
  | "settled_speaker_diarization"
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
  if (details.refineSpeakerDiarization) {
    reasons.push("settled_speaker_diarization");
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
  const liveTranscriptComplete =
    details.liveTranscriptionActive &&
    !details.needsBatchRepair &&
    !details.transcriptWriteFailed;

  if (liveTranscriptComplete && !details.refineSpeakerDiarization) {
    return "enhance_only" as const;
  }

  if (!!details.audioPath && canRunBatch) {
    return "batch_then_enhance" as const;
  }

  if (liveTranscriptComplete) {
    return "enhance_only" as const;
  }

  return "none" as const;
}

export function useCaptureLifecycle(sessionId: string) {
  const session = useSession(sessionId);
  const auth = useAuth();
  const authRef = useRef(auth);
  authRef.current = auth;
  const transcriptExistence = useSessionTranscriptExistence(sessionId);
  const participantHumanIds = useSessionParticipantHumanIds(sessionId);
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const rememberSpeakers = useConfigValue("remember_speakers") === true;
  const {
    conn,
    isReady: connectionReady,
    localBatchDiarizationAvailable,
  } = useSTTConnection();
  const runBatch = useRunBatch(sessionId);
  const setBatchTranscriptionPending = useListener(
    (state) => state.setBatchTranscriptionPending,
  );

  const runBatchRef = useRef(runBatch);
  const canRunBatchRef = useRef(canRunBatchTranscription(conn));
  const localBatchDiarizationAvailableRef = useRef(
    localBatchDiarizationAvailable,
  );
  const stopMeetingChatCaptureRef = useRef<(() => Promise<void>) | null>(null);
  runBatchRef.current = runBatch;
  canRunBatchRef.current = canRunBatchTranscription(conn);
  localBatchDiarizationAvailableRef.current = localBatchDiarizationAvailable;

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
    (
      recoveredMarker?: CaptureLifecycleMarker,
      startedAutomatically = false,
      pendingMarker?: CaptureLifecycleMarker,
    ) => {
      const inheritedCaptures: InheritedCapture[] =
        recoveredMarker?.inheritedCaptures ??
        (pendingMarker
          ? [
              ...(pendingMarker.inheritedCaptures ?? []),
              {
                transcriptId: pendingMarker.transcriptId,
                startedAt: pendingMarker.startedAt,
                createdAt: pendingMarker.createdAt,
                ownerUserId: pendingMarker.ownerUserId,
                memo: pendingMarker.memo,
                ...(pendingMarker.retainAudio !== undefined
                  ? { retainAudio: pendingMarker.retainAudio }
                  : {}),
                ...(pendingMarker.provider
                  ? { provider: pendingMarker.provider }
                  : {}),
                ...(pendingMarker.model ? { model: pendingMarker.model } : {}),
              },
            ]
          : []);
      const inheritedAudioPending = inheritedCaptures.some(
        (capture) => capture.retainAudio === false,
      );
      const clearInheritedIncomplete = async () => {
        for (const capture of inheritedCaptures)
          await clearIncompleteCapture(sessionId, capture.transcriptId);
      };
      let inheritedOnly = recoveredMarker?.inheritedOnly === true;
      let usesChunkedAudio =
        !recoveredMarker || recoveredMarker.chunkedAudio === true;
      const automatic = recoveredMarker
        ? recoveredMarker.automatic === true
        : startedAutomatically;
      const initialTitle = recoveredMarker
        ? recoveredMarker.initialTitle
        : session?.title;
      const existingAudioPromise = recoveredMarker
        ? Promise.resolve(recoveredMarker.preserveExistingAudio ?? true)
        : automatic
          ? fsSyncCommands
              .audioExist(sessionId)
              .then((result) => (result.status === "ok" ? result.data : true))
              .catch(() => true)
          : Promise.resolve(true);
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
      const retainAudio =
        recoveredMarker?.retainAudio ??
        (audioRetention !== "none" ||
          requiresRetainedBatchAudio(provider, model));
      // Persisted on the marker so restarts still run the full-file pass
      // instead of the chunks released during the meeting.
      let batchFromRetainedAudio =
        retainAudio &&
        (requiresRetainedBatchAudio(provider, model) ||
          recoveredMarker?.postStopBatch === true);
      const hasMultipleRemoteParticipants =
        new Set(
          participantHumanIds.filter(
            (humanId) => humanId && humanId !== ownerUserId,
          ),
        ).size > 1;
      const shouldUseLocalBatchForSpeakerDiarization = () =>
        hasMultipleRemoteParticipants &&
        localBatchDiarizationAvailableRef.current &&
        isRealtimeLocalModel(model);
      const shouldRefineSpeakerDiarization = () =>
        hasMultipleRemoteParticipants &&
        ((provider === "anarlog" && model === "cloud") ||
          shouldUseLocalBatchForSpeakerDiarization());
      const cloudsyncLeaseKey = `${sessionId}:${transcriptId}`;
      let pendingSummaryMode = recoveredMarker?.summaryMode;
      const refreshSummaryAfterRepair =
        recoveredMarker?.refreshSummaryAfterRepair ?? false;
      let completionTracked = false;
      let capturePhase =
        recoveredMarker?.phase ??
        (recoveredMarker?.summaryMode ? "finalizing" : "capturing");
      const existingAudioDurationPromise = recoveredMarker
        ? Promise.resolve(recoveredMarker.audioOffsetMs)
        : preserveExistingTranscript
          ? getExistingAudioDurationMs(sessionId)
          : Promise.resolve(0);
      let transcriptWriteError: unknown;
      let cloudsyncLeaseActive = false;
      let cloudsyncLeaseAcquire: Promise<void> | null = null;
      let cloudsyncLeaseRelease: Promise<void> | null = null;
      let cloudsyncLeaseRetired = false;
      let recoveryPending = Boolean(recoveredMarker);
      let recoveryStateCleared = false;
      let batchTranscriptionPending = false;
      const updateBatchTranscriptionPending = (pending: boolean) => {
        if (batchTranscriptionPending === pending) {
          return;
        }
        batchTranscriptionPending = pending;
        setBatchTranscriptionPending(sessionId, pending);
      };
      const handoffCloudsyncLease = () => {
        cloudsyncLeaseRetired = true;
        cloudsyncLeaseActive = false;
        cloudsyncLeaseAcquire = null;
        cloudsyncLeaseRelease = null;
      };
      const endCloudsyncLease = () => {
        if (cloudsyncLeaseRelease) {
          return cloudsyncLeaseRelease;
        }
        if (!cloudsyncLeaseActive) {
          return Promise.resolve();
        }
        // A release racing an in-flight acquisition would end the lease before
        // the native side registers it and leave CloudSync paused for good.
        const acquisitionSettled =
          cloudsyncLeaseAcquire?.catch(() => undefined) ?? Promise.resolve();
        cloudsyncLeaseRelease = acquisitionSettled
          .then(() =>
            releaseCloudsyncActivityEventually(
              CLOUDSYNC_CAPTURE_ACTIVITY,
              cloudsyncLeaseKey,
            ),
          )
          .then(
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
      const releaseCloudsyncLease = () => {
        cloudsyncLeaseRetired = true;
        return endCloudsyncLease();
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
            await endCloudsyncLease();
          }
          throw error;
        }
      };
      // Deferring CloudSync is bookkeeping around the capture, not a
      // prerequisite for it. The native drain is bounded and its first timeout
      // is usually a sync round that is still yielding, so keep trying in the
      // background while the capture is alive; recording never waits on this.
      const deferCloudsync = async () => {
        for (let attempt = 1; ; attempt += 1) {
          if (cloudsyncLeaseRetired) {
            return;
          }
          try {
            await acquireCloudsyncLease();
            return;
          } catch (error) {
            if (cloudsyncLeaseRetired) {
              return;
            }
            if (attempt >= CLOUDSYNC_CAPTURE_LEASE_ATTEMPTS) {
              console.error(
                "[listener] failed to defer CloudSync for capture",
                error,
              );
              trackAnalyticsEvent("capture_cloudsync_deferral_failed");
              return;
            }
            console.warn(
              `[listener] CloudSync capture deferral attempt ${attempt} failed, retrying`,
              error,
            );
          }
        }
      };
      const transcriptPersistence = createNativeTranscriptPersistence({
        sessionId,
        transcriptId,
        onPersisted: (status) => {
          if (status.transcript_created) transcriptCreated = true;
          if (status.transcript_created) transcriptTouched = true;
          if (status.persisted_through_ms != null)
            audioRecovery.persistedThrough(status.persisted_through_ms);
          if (!transcriptPersistence.hasPendingFailure())
            transcriptWriteError = undefined;
        },
        onError: (error) => {
          transcriptWriteError = error;
          audioRecovery.persistenceFailed();
          toast.error("Your transcript could not be saved", {
            id: `transcript-storage-${sessionId}`,
            description:
              "Free up disk space. Anarlog will try to recover the missing text while this meeting is still recording.",
          });
          console.error("[listener] failed to persist transcript", error);
        },
        afterFlush: () =>
          persistTranscriptWrite(() =>
            flushLiveTranscriptDeltasToDatabase(transcriptId),
          ),
      });
      const earliestStartedAt = Math.min(
        startedAt,
        ...inheritedCaptures.map((capture) => capture.startedAt),
      );
      const inheritedCaptureFor = (chunk: RecoveryAudioChunk) => {
        if (chunk.capture_started_at >= startedAt - 5_000) return undefined;
        return inheritedCaptures
          .filter(
            (capture) => chunk.capture_started_at >= capture.startedAt - 5_000,
          )
          .reduce<InheritedCapture | undefined>(
            (latest, capture) =>
              !latest || capture.startedAt > latest.startedAt
                ? capture
                : latest,
            undefined,
          );
      };
      const listRecoveryChunks = async () => {
        const result =
          await transcriptionCommands.listCaptureAudioChunks(sessionId);
        if (result.status === "error") throw new Error(result.error);
        return result.data.filter(
          (chunk) => chunk.capture_started_at >= earliestStartedAt - 5_000,
        );
      };
      const audioRecovery = createCaptureAudioRecovery({
        startedAt,
        list: listRecoveryChunks,
        inherited: (chunk) => inheritedCaptureFor(chunk) !== undefined,
        acknowledge: async (chunk) => {
          const result =
            await transcriptionCommands.acknowledgeCaptureAudioChunk(
              sessionId,
              chunk.id,
            );
          if (result.status === "error") throw new Error(result.error);
        },
        flush: async () => {
          await transcriptPersistence.flush();
          if (transcriptPersistence.hasPendingFailure())
            audioRecovery.persistenceFailed();
          else transcriptWriteError = undefined;
        },
        repair: async (chunk, requestedIntervals, signal) => {
          const inherited = inheritedCaptureFor(chunk);
          const target = inherited ?? {
            transcriptId,
            startedAt,
            createdAt,
            ownerUserId,
            memo: memoMd,
            provider,
            model,
          };
          const intervals = inherited
            ? [chunkInterval(chunk, target.startedAt)]
            : requestedIntervals;
          const beforeRepair = await getTranscriptRecord(target.transcriptId);
          const audioOffset =
            chunk.capture_started_at - target.startedAt + chunk.audio_start_ms;
          try {
            await runBatchRef.current(chunk.path, {
              signal,
              deferAudioFinalization: true,
              notifyOnCompletion: false,
              recovery: {
                persist: async (words, hints) => {
                  await transcriptPersistence.flush();
                  signal.throwIfAborted();
                  if (transcriptPersistence.hasPendingFailure())
                    throw new Error(
                      "Waiting for transcript storage to recover",
                    );
                  if (!words.length) return;
                  const created = inherited
                    ? await transcriptExists(target.transcriptId)
                    : (transcriptCreated ||=
                        await transcriptExists(transcriptId));
                  if (!created) {
                    await createLiveTranscript(
                      {
                        id: target.transcriptId,
                        sessionId,
                        ownerUserId: target.ownerUserId,
                        createdAt: target.createdAt,
                        startedAt: target.startedAt,
                        memo: target.memo,
                        source: "live_capture",
                        provider: target.provider ?? undefined,
                        model: target.model ?? undefined,
                      },
                      { new_words: [], replaced_ids: [], partials: [] },
                    );
                    if (!inherited) transcriptCreated = true;
                  }
                  const shifted = words.map((word) => ({
                    ...word,
                    start_ms: Number(word.start_ms) + audioOffset,
                    end_ms: Number(word.end_ms) + audioOffset,
                  }));
                  const reconciledHints = beforeRepair
                    ? await reconcileRefinedSpeakerClusters(
                        beforeRepair,
                        shifted,
                        hints,
                      )
                    : hints;
                  await persistTranscriptWrite(() =>
                    appendRecoveredTranscriptWords(
                      target.transcriptId,
                      shifted,
                      reconciledHints,
                      intervals,
                      beforeRepair?.words ?? [],
                    ),
                  );
                  if (!inherited) transcriptTouched = true;
                },
              },
            });
          } finally {
            listenerStore.getState().clearBatchSession(`${sessionId}:recovery`);
          }
        },
      });
      const restoreAudioRecovery = async () => {
        const result = await transcriptionCommands
          .getCaptureAudioGaps(sessionId)
          .catch((error) => {
            console.warn(
              "[listener] failed to restore capture audio gaps",
              error,
            );
            audioRecovery.recoverPending();
            return undefined;
          });
        if (!result) return;
        if (result.status === "error") {
          console.warn(
            "[listener] failed to restore capture audio gaps",
            result.error,
          );
          audioRecovery.recoverPending();
          return;
        }
        const ledger = result.data;
        const maxNativeStartDelayMs = 60_000;
        if (
          !ledger ||
          ledger.capture_started_at_ms < startedAt - 5_000 ||
          ledger.capture_started_at_ms > startedAt + maxNativeStartDelayMs
        ) {
          audioRecovery.recoverPending();
          return;
        }
        audioRecovery.restore({
          gaps: ledger.gaps.map((gap) => ({
            start: gap.start_ms - startedAt,
            end: gap.end_ms - startedAt,
          })),
          ...(ledger.open_gap_started_at_ms == null
            ? {}
            : { openGapStart: ledger.open_gap_started_at_ms - startedAt }),
          awaitingConnection: ledger.awaiting_connection,
          storageFailed: ledger.storage_failed,
          ...(ledger.confirmed_through_ms == null
            ? {}
            : { confirmedThrough: ledger.confirmed_through_ms }),
        });
      };
      let recoveryUnlisten: (() => void)[] = [];
      let recoveryListening: Promise<void> | undefined;
      let credentialTimer: ReturnType<typeof setTimeout> | undefined;
      let refreshCredentialsActive = false;
      const refreshCredentials = async () => {
        if (!refreshCredentialsActive) return;
        try {
          const current = await authRef.current.getSessionForRequest();
          if (refreshCredentialsActive && current?.access_token) {
            await transcriptionCommands.updateCaptureCredentials(
              sessionId,
              current.access_token,
            );
          }
        } catch (error) {
          console.warn("[listener] capture credential refresh deferred", error);
        } finally {
          if (refreshCredentialsActive)
            credentialTimer = setTimeout(
              () => void refreshCredentials(),
              30_000,
            );
        }
      };
      const startAudioRecovery = () =>
        (recoveryListening ??= Promise.all([
          transcriptionEvents.captureLifecycleEvent.listen(({ payload }) => {
            if (payload.session_id !== sessionId) return;
            if (payload.type === "started") {
              if (payload.live_transcription_active) audioRecovery.connected();
              else if (batchFromRetainedAudio) audioRecovery.batchOnly(true);
              else if (!payload.requested_live_transcription) {
                batchFromRetainedAudio = retainAudio;
                audioRecovery.batchOnly(retainAudio);
              } else audioRecovery.interrupted();
            }
          }),
          transcriptionEvents.captureStatusEvent.listen(({ payload }) => {
            if (payload.session_id !== sessionId) return;
            if (payload.type === "connected") audioRecovery.connected();
            else if (payload.type === "connection_error")
              audioRecovery.interrupted();
            else if (
              payload.type === "audio_error" &&
              payload.error.startsWith("audio_storage_")
            )
              audioRecovery.storageFailed();
          }),
        ]).then(async (unlisten) => {
          recoveryUnlisten = unlisten;
          audioRecovery.start();
          if (recoveredMarker && !batchFromRetainedAudio && !inheritedOnly)
            await restoreAudioRecovery();
          if (batchFromRetainedAudio || inheritedOnly)
            audioRecovery.batchOnly(true);
          if (provider === "anarlog" && model === "cloud") {
            refreshCredentialsActive = true;
            credentialTimer = setTimeout(
              () => void refreshCredentials(),
              1_000,
            );
          }
        }));
      const stopAudioRecovery = async () => {
        await recoveryListening;
        refreshCredentialsActive = false;
        clearTimeout(credentialTimer);
        recoveryUnlisten.forEach((unlisten) => unlisten());
        recoveryUnlisten = [];
        const recovery = await audioRecovery.stop().catch((error) => {
          console.error("[listener] audio recovery did not finish", error);
          return { incomplete: true };
        });
        if (retainAudio || recovery.incomplete) return recovery;
        const result =
          await transcriptionCommands.deleteTranscribedCaptureAudio(sessionId);
        if (result.status === "error") throw new Error(result.error);
        return { incomplete: !result.data };
      };
      // Without the full recording, the chunks are the only audio left to
      // transcribe, so they must be repaired instead of released.
      const repairChunksWithoutRecording = async (audioPath: string | null) => {
        await recoveryListening;
        if (!batchFromRetainedAudio || audioPath) return;
        batchFromRetainedAudio = false;
        audioRecovery.batchOnly(false);
        audioRecovery.recoverPending();
      };
      const marker = async (): Promise<CaptureLifecycleMarker> => ({
        version: 1,
        chunkedAudio: usesChunkedAudio,
        retainAudio,
        ...(batchFromRetainedAudio ? { postStopBatch: true } : {}),
        phase: capturePhase,
        sessionId,
        transcriptId,
        startedAt,
        createdAt,
        audioOffsetMs: await existingAudioDurationPromise,
        preserveExistingTranscript,
        automatic,
        preserveExistingAudio: await existingAudioPromise,
        initialTitle,
        ownerUserId,
        memo: memoMd,
        ...(provider ? { provider } : {}),
        ...(model ? { model } : {}),
        ...(pendingSummaryMode ? { summaryMode: pendingSummaryMode } : {}),
        ...(refreshSummaryAfterRepair
          ? { refreshSummaryAfterRepair: true }
          : {}),
        ...(inheritedCaptures.length > 0 ? { inheritedCaptures } : {}),
        ...(inheritedOnly ? { inheritedOnly: true } : {}),
      });
      const releaseMarker = async () => {
        const inheritedAudioRemains =
          inheritedCaptures.length > 0 &&
          (await listRecoveryChunks().then(
            (chunks) =>
              chunks.some((chunk) => inheritedCaptureFor(chunk) !== undefined),
            () => true,
          ));
        if (!inheritedAudioRemains) {
          await clearCaptureLifecycleMarker(sessionId, transcriptId);
          return;
        }
        inheritedOnly = true;
        capturePhase = "finalizing";
        await saveCaptureLifecycleMarker(await marker());
        await requestCaptureRecoverySafely(sessionId);
      };
      const finalizeStoppedInner = async (
        details: Parameters<OnStoppedCallback>[1],
        requestRecoveryOnFailure: boolean,
      ) => {
        toast.dismiss("recording-without-transcription");
        toast.dismiss("recording-with-limited-transcription-languages");
        toast.dismiss("live-transcription-stalled");
        toast.dismiss("meeting-disclosure-send-failed");
        const sessionWasDeleted = async () => {
          try {
            return await isSessionDeleted(sessionId);
          } catch (error) {
            console.warn(
              "[listener] failed to check whether the session was deleted",
              error,
            );
            return false;
          }
        };
        const notifyFailure = async (message: string, id: string) => {
          if (requestRecoveryOnFailure && !(await sessionWasDeleted())) {
            toast.error(message, { id });
          }
        };
        const requestRecovery = async () => {
          recoveryPending = true;
          if (await sessionWasDeleted()) {
            try {
              await persistTranscriptWrite(() =>
                clearCaptureLifecycleMarker(sessionId, transcriptId),
              );
              recoveryPending = false;
              recoveryStateCleared = true;
            } catch (error) {
              console.error(
                "[listener] failed to clear deleted session capture state",
                error,
              );
              if (requestRecoveryOnFailure) {
                await requestCaptureRecoverySafely(sessionId);
              }
            }
            return;
          }
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
        await transcriptPersistence.flush();
        if (
          consumePrimaryDeviceYield(sessionId) &&
          !preserveExistingTranscript &&
          inheritedCaptures.length === 0 &&
          !(await existingAudioPromise)
        ) {
          try {
            if (details.audioPath) {
              const deleted = await enqueueSessionAudioOperation(
                sessionId,
                () => fsSyncCommands.audioDelete(sessionId),
              );
              if (deleted.status !== "ok") {
                throw new Error(deleted.error);
              }
            }
            if (await transcriptExists(transcriptId)) {
              await softDeleteTranscript(transcriptId);
            }
            await releaseMarker();
            recoveryPending = false;
            recoveryStateCleared = true;
            return;
          } catch (error) {
            console.error(
              "[listener] failed to discard capture recorded on another device",
              error,
            );
          }
        }
        if (
          details.audioPath &&
          (await discardEmptyAutomaticCapture({
            sessionId,
            automatic,
            preserveExistingAudio: await existingAudioPromise,
            preserveExistingTranscript,
            initialTitle: initialTitle ?? undefined,
            transcriptTouched,
            transcriptionComplete:
              (!details.requestedLiveTranscription ||
                details.liveTranscriptionActive) &&
              !details.needsBatchRepair &&
              !transcriptWriteError,
          }))
        ) {
          await releaseMarker();
          recoveryPending = false;
          recoveryStateCleared = true;
          return;
        }
        trackSessionCompletion(
          details,
          recoveredMarker ? "recovered_capture_stopped" : "capture_stopped",
        );
        if (details.audioPath) {
          try {
            await enqueueSessionAudioOperation(sessionId, () =>
              catalogLocalSessionAudio(sessionId),
            );
          } catch (error) {
            console.error("[listener] failed to catalog recorded audio", error);
          }
        }
        transcriptCreated ??= await transcriptExists(transcriptId);
        const useLocalBatchForSpeakerDiarization =
          shouldUseLocalBatchForSpeakerDiarization();
        const refineSpeakerDiarization =
          !usesChunkedAudio && shouldRefineSpeakerDiarization();
        if (transcriptCreated) {
          try {
            await waitForSessionSearchIndex(sessionId);
          } catch (error) {
            console.warn(
              "[listener] search index finalization is still pending",
              error,
            );
          }
        }

        const postCaptureAction = pendingSummaryMode
          ? ("enhance_only" as const)
          : getPostCaptureAction(
              {
                ...details,
                refineSpeakerDiarization,
                transcriptWriteFailed: Boolean(transcriptWriteError),
              },
              canRunBatchRef.current &&
                (!usesChunkedAudio || batchFromRetainedAudio),
            );
        const repairReasons = pendingSummaryMode
          ? []
          : getPostCaptureRepairReasons({
              ...details,
              refineSpeakerDiarization,
              transcriptWriteFailed: Boolean(transcriptWriteError),
            });

        let batchCompleted = false;
        if (postCaptureAction === "batch_then_enhance") {
          updateBatchTranscriptionPending(true);
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
              notifyOnCompletion: !details.liveTranscriptionActive,
              ...(useLocalBatchForSpeakerDiarization
                ? {
                    provider: "soniqo",
                    model: "soniqo-parakeet-batch",
                    baseUrl: "soniqo://local",
                    apiKey: "",
                  }
                : {}),
              promotion:
                preserveExistingTranscript || transcriptCreated
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
              await persistTranscriptWrite(() =>
                clearCaptureLifecycleMarker(sessionId, transcriptId),
              );
              recoveryPending = false;
              recoveryStateCleared = true;
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
              await notifyFailure(
                "Anarlog could not finish saving the transcript. The recording was kept so you can try again.",
                "post-capture-transcript-incomplete",
              );
            } else {
              await notifyFailure(
                "Post-meeting transcription failed. The recording was kept so you can try again.",
                "post-capture-batch-failed",
              );
            }
            if (isTerminalTranscriptionError(error)) {
              try {
                await clearCaptureLifecycleMarker(sessionId, transcriptId);
                recoveryPending = false;
                recoveryStateCleared = true;
              } catch (clearError) {
                console.error(
                  "[listener] failed to stop automatic capture recovery",
                  clearError,
                );
                await requestRecovery();
              }
              return;
            }
            await requestRecovery();
            return;
          }
        }

        if (
          transcriptWriteError &&
          postCaptureAction !== "batch_then_enhance"
        ) {
          await notifyFailure(
            details.audioPath
              ? "Anarlog could not finish saving the transcript. The recording was kept so you can try again."
              : "Anarlog could not save part of the live transcript.",
            details.audioPath
              ? "post-capture-transcript-incomplete"
              : "live-transcript-persist-failed",
          );
        }

        const emptyFreshCapture =
          !recoveredMarker &&
          !details.audioPath &&
          !(usesChunkedAudio && details.needsBatchRepair) &&
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

        // Batch repair already requests attention when runBatchSession
        // finishes; a recovered summary-only pass has no new transcript.
        if (
          !batchCompleted &&
          !pendingSummaryMode &&
          (transcriptTouched || preserveExistingTranscript)
        ) {
          void playCompletionSound();
          void requestAppAttention();
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
            (refreshSummaryAfterRepair ||
            (preserveExistingTranscript &&
              (transcriptTouched || batchCompleted))
              ? "regenerate"
              : batchCompleted
                ? "refresh"
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
              await notifyFailure(
                "The transcript was saved, but Anarlog could not start the summary. Try generating it again.",
                "post-capture-summary-failed",
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
            await notifyFailure(
              "The transcript was saved, but Anarlog could not start the summary. Try generating it again.",
              "post-capture-summary-failed",
            );
          }
        }

        const recoveryComplete =
          !retainAudio ||
          (!details.audioPath && !transcriptWriteError) ||
          (transcriptIsComplete && summaryScheduled);
        if (!recoveryComplete) {
          await requestRecovery();
          return;
        }

        try {
          if (details.audioPath && transcriptIsComplete) {
            await maybeExtractVoiceprintCandidates({
              enabled: rememberSpeakers,
              sessionId,
              transcriptId,
              audioPath: details.audioPath,
            });
            await persistTranscriptWrite(() =>
              markSessionAudioTranscriptionComplete(sessionId),
            );
          }
          await releaseMarker();
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

        // Native capture cleanup and the retention sweep enforce deletion even
        // when legacy post-capture processing cannot finish.
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
          if (!transcriptPersistence.hasPendingFailure())
            transcriptPersistence.dispose();
          updateBatchTranscriptionPending(false);
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
      const onStopped: OnStoppedCallback = async (_sessionId, details) => {
        usesChunkedAudio = details.chunkedAudio === true;
        if (usesChunkedAudio) {
          await transcriptPersistence.flush();
          if (transcriptPersistence.hasPendingFailure())
            audioRecovery.persistenceFailed();
          // The native stop is authoritative for whether live STT ran.
          if (!requiresRetainedBatchAudio(provider, model))
            batchFromRetainedAudio =
              retainAudio && details.requestedLiveTranscription === false;
          await repairChunksWithoutRecording(details.audioPath);
          const recovery = await stopAudioRecovery().catch((error) => {
            console.error(
              "[listener] failed to delete transcribed audio",
              error,
            );
            details = { ...details, audioDeletionFailed: true };
            return { incomplete: false };
          });
          details = {
            ...details,
            needsBatchRepair: recovery.incomplete,
            liveTranscriptionActive:
              !recovery.incomplete && !batchFromRetainedAudio,
          };
          const audioKeptForTranscription =
            recovery.incomplete && (!retainAudio || inheritedAudioPending);
          if (!recovery.incomplete)
            await clearInheritedIncomplete().catch((error) =>
              console.error(
                "[listener] failed to clear earlier capture status",
                error,
              ),
            );
          if (recovery.incomplete || details.audioDeletionFailed) {
            await saveIncompleteCapture(
              sessionId,
              transcriptId,
              false,
              details.audioDeletionFailed ?? false,
              audioKeptForTranscription,
            ).catch((error) =>
              console.error(
                "[listener] failed to save incomplete capture status",
                error,
              ),
            );
          }
          if (details.audioDeletionFailed) {
            toast.error("Audio could not be deleted", {
              id: `audio-deletion-${sessionId}`,
              duration: Infinity,
              description:
                "Anarlog could not remove the temporary audio. Cleanup will be retried automatically.",
            });
          } else if (audioKeptForTranscription) {
            toast.warning("Audio kept to finish your transcript", {
              id: `capture-incomplete-${sessionId}`,
              duration: Infinity,
              description:
                "Part of this meeting could not be transcribed yet, so Anarlog kept its temporary audio on purpose. It will be deleted automatically once transcription succeeds.",
            });
          }
        } else {
          audioRecovery.cancel();
          refreshCredentialsActive = false;
          clearTimeout(credentialTimer);
          recoveryUnlisten.forEach((unlisten) => unlisten());
        }
        if (!retainAudio) details = { ...details, audioPath: null };
        recoveryPending = false;
        markExpectedPostStopBatch(details);
        return finalizeStopped(details, true);
      };
      const markExpectedPostStopBatch = (
        details: Parameters<OnStoppedCallback>[1],
      ) => {
        if (
          (!usesChunkedAudio || batchFromRetainedAudio) &&
          !pendingSummaryMode &&
          details.audioPath &&
          canRunBatchRef.current &&
          (!details.liveTranscriptionActive ||
            details.needsBatchRepair ||
            shouldRefineSpeakerDiarization())
        ) {
          updateBatchTranscriptionPending(true);
        }
      };
      const recoverStopped: OnStoppedCallback = async (_sessionId, details) => {
        if (inheritedOnly) {
          audioRecovery.batchOnly(true);
          const recovery = await stopAudioRecovery();
          if (recovery.incomplete)
            throw new Error("earlier capture audio is still pending");
          await clearIncompleteCapture(sessionId, transcriptId);
          await clearInheritedIncomplete();
          toast.dismiss(`capture-incomplete-${sessionId}`);
          const service = getEnhancerService();
          if (!service) await requestMainAutoEnhance(sessionId, "regenerate");
          else await service.requestAutoEnhance(sessionId, "regenerate");
          await clearCaptureLifecycleMarker(sessionId, transcriptId);
          await releaseCloudsyncLease();
          return;
        }
        if (usesChunkedAudio) {
          await repairChunksWithoutRecording(details.audioPath);
          if (!batchFromRetainedAudio) await restoreAudioRecovery();
          const recovery = await stopAudioRecovery();
          details = {
            ...details,
            needsBatchRepair: recovery.incomplete,
            liveTranscriptionActive:
              !recovery.incomplete && !batchFromRetainedAudio,
            ...(!retainAudio ? { audioPath: null } : {}),
          };
        }
        if (usesChunkedAudio && !details.needsBatchRepair) {
          await clearIncompleteCapture(sessionId, transcriptId);
          await clearInheritedIncomplete();
          toast.dismiss(`capture-incomplete-${sessionId}`);
        } else if (usesChunkedAudio)
          await saveIncompleteCapture(
            sessionId,
            transcriptId,
            false,
            false,
            !retainAudio || inheritedAudioPending,
          );
        markExpectedPostStopBatch(details);
        return finalizeStopped(details, false);
      };

      const handlePersist: LiveTranscriptPersistCallback = (delta) => {
        if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
          return;
        }

        transcriptTouched = true;
      };

      return {
        acquireCloudsyncLease,
        deferCloudsync,
        handlePersist,
        liveTranscript: {
          transcript_id: transcriptId,
          owner_user_id: ownerUserId,
          created_at: createdAt,
          started_at_ms: startedAt,
          memo: memoMd,
          provider: provider ?? null,
          model: model ?? null,
        } satisfies LiveTranscriptTarget,
        onStopped,
        recoverStopped,
        ready: Promise.all([
          existingAudioDurationPromise,
          existingAudioPromise,
        ]).then(() => undefined),
        startAudioRecovery,
        persistMarker: async (options?: { batchOnly?: boolean }) => {
          if (options?.batchOnly && retainAudio) batchFromRetainedAudio = true;
          await startAudioRecovery();
          await persistTranscriptWrite(async () => {
            const next = await marker();
            await (pendingMarker
              ? saveCaptureLifecycleMarker(next, pendingMarker.transcriptId)
              : saveCaptureLifecycleMarker(next));
          });
        },
        cleanupFailedStart: async () => {
          refreshCredentialsActive = false;
          clearTimeout(credentialTimer);
          audioRecovery.cancel();
          recoveryUnlisten.forEach((unlisten) => unlisten());
          await transcriptPersistence.flush();
          transcriptPersistence.dispose();
          if (pendingMarker)
            await saveCaptureLifecycleMarker(pendingMarker, transcriptId);
          else await clearCaptureLifecycleMarker(sessionId, transcriptId);
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
      participantHumanIds,
      rememberSpeakers,
      session?.raw_md,
      session?.user_id,
      sessionId,
      setBatchTranscriptionPending,
      stopMeetingChatTasks,
      transcriptExistence,
    ],
  );

  return {
    conn,
    connectionReady,
    createCaptureLifecycle,
    session,
    setStopMeetingChatCapture,
    stopMeetingChatTasks,
  };
}
