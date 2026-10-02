import { t } from "@lingui/core/macro";
import type { StoreApi } from "zustand";

import { commands as notificationCommands } from "@anlg/plugin-notification";
import {
  type BatchErrorCode,
  type TranscriptionEvent,
  type TranscriptionParams,
  type TranscriptionSession,
  commands as transcriptionCommands,
  events as transcriptionEvents,
} from "@anlg/plugin-transcription";

import {
  EMPTY_BATCH_TRANSCRIPT_ERROR,
  type BatchActions,
  type BatchState,
} from "./batch";

import { trackAnalyticsEvent } from "~/analytics";
import { requestAppAttention } from "~/shared/app-attention";
import { playCompletionSound } from "~/shared/completion-sound";
import { shouldShowNotification } from "~/shared/notification-policy";
import { isAppWindowInactive } from "~/shared/window-activity";
import { createBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";
import { BatchResponseProcessingError } from "~/stt/batch-response-processing-error";

type BatchStore = BatchActions & BatchState;

const SYNTHETIC_BATCH_PROGRESS_INITIAL = 0.06;
const SYNTHETIC_BATCH_PROGRESS_MAX = 0.88;
const SYNTHETIC_BATCH_PROGRESS_INTERVAL_MS = 800;
const SYNTHETIC_BATCH_PROGRESS_TIME_CONSTANT_MS = 32_000;
const BATCH_COMPLETED_NOTIFICATION_TIMEOUT_SECONDS = 15;
const SESSION_ALREADY_RUNNING_ERROR = "session already running";
const OPENAI_PROGRESSIVE_BATCH_MODELS = new Set([
  "gpt-transcribe",
  "gpt-4o-transcribe",
  "gpt-4o-mini-transcribe",
  "gpt-4o-mini-transcribe-2025-12-15",
]);

export async function showBatchCompletedNotification(
  sessionId: string,
  options?: { force?: boolean },
) {
  if (!options?.force) {
    if (!(await isAppWindowInactive())) {
      return;
    }
    if (
      !(await shouldShowNotification("notification_transcription_complete"))
    ) {
      return;
    }
  }

  try {
    const result = await notificationCommands.showNotification({
      key: createBatchCompletedNotificationKey(sessionId),
      title: t`Transcription complete`,
      message: t`Your transcript is ready.`,
      timeout: {
        secs: BATCH_COMPLETED_NOTIFICATION_TIMEOUT_SECONDS,
        nanos: 0,
      },
      source: { type: "session", session_id: sessionId },
      start_time: null,
      participants: null,
      event_details: null,
      action_label: t`Open Anarlog`,
      action_variant: null,
      options: null,
      footer: null,
      icon: null,
    });

    if (result.status === "error") {
      console.error(
        "[runBatch] failed to show completion notification",
        result.error,
      );
    }
  } catch (error) {
    console.error("[runBatch] failed to show completion notification", error);
  }
}

export async function notifyBatchCompleted(sessionId: string) {
  await showBatchCompletedNotification(sessionId);
  void playCompletionSound();
  void requestAppAttention();
}

export const runBatchSession = async <T extends BatchStore>(
  get: StoreApi<T>["getState"],
  sessionId: string,
  params: TranscriptionParams,
  options?: {
    notifyOnCompletion?: boolean;
    signal?: AbortSignal;
    recovery?: boolean;
  },
) => {
  const resumeFromRecovery = get().batch[sessionId]?.recovered === true;
  get().handleBatchStarted(sessionId);

  let unlisten: (() => void) | undefined;
  let syntheticProgressTimer: ReturnType<typeof setInterval> | undefined;
  let settled = false;

  const stopSyntheticProgress = () => {
    if (syntheticProgressTimer) {
      clearInterval(syntheticProgressTimer);
      syntheticProgressTimer = undefined;
    }
  };

  const cleanup = (clearSession = true) => {
    stopSyntheticProgress();

    if (unlisten) {
      unlisten();
      unlisten = undefined;
    }

    get().clearBatchPersist(sessionId);

    if (clearSession) {
      get().clearBatchSession(sessionId);
    }
  };

  let startedAt = Date.now();
  if (shouldUseSyntheticBatchProgress(params)) {
    get().updateBatchProgress(sessionId, SYNTHETIC_BATCH_PROGRESS_INITIAL);
    syntheticProgressTimer = setInterval(() => {
      get().updateBatchProgress(
        sessionId,
        syntheticBatchProgress(Date.now() - startedAt),
      );
    }, SYNTHETIC_BATCH_PROGRESS_INTERVAL_MS);
  }

  const resolveSuccess = (
    output: {
      response: Parameters<BatchStore["handleBatchResponse"]>[1];
    },
    resolve: () => void,
    reject: (reason?: unknown) => void,
  ) => {
    if (settled) {
      return;
    }

    settled = true;

    let emptyResponse = false;
    try {
      const handled = get().handleBatchResponse(sessionId, output.response);
      if (handled === false) {
        emptyResponse = true;
        throw new Error(EMPTY_BATCH_TRANSCRIPT_ERROR);
      }
      trackAnalyticsEvent("transcription_completed", {
        mode: "batch",
        provider: params.provider,
      });
      cleanup();
    } catch (error) {
      console.error("[runBatch] error handling batch response", error);
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      get().handleBatchFailed(sessionId, errorMessage);
      if (!(options?.recovery && emptyResponse)) {
        trackAnalyticsEvent("transcription_failed", {
          mode: "batch",
          failure_stage: "persist",
        });
      }
      cleanup(false);
      reject(
        error instanceof Error && error.message === EMPTY_BATCH_TRANSCRIPT_ERROR
          ? error
          : new BatchResponseProcessingError(error),
      );
      return;
    }

    resolve();
  };

  const rejectFailure = (
    error: unknown,
    reject: (reason?: unknown) => void,
    options?: {
      clearSession?: boolean;
      terminalReason?: "failed" | "timed_out";
      errorCode?: BatchErrorCode;
    },
  ) => {
    if (settled) {
      return;
    }

    settled = true;

    const errorMessage = error instanceof Error ? error.message : String(error);
    get().handleBatchFailed(
      sessionId,
      errorMessage,
      options?.terminalReason,
      options?.errorCode,
    );
    trackAnalyticsEvent("transcription_failed", {
      mode: "batch",
      failure_stage: options?.terminalReason ?? "provider",
      error_code: options?.errorCode ?? "unknown",
      provider: params.provider,
    });
    cleanup(options?.clearSession ?? false);
    reject(error);
  };

  const rejectStopped = (reject: (reason?: unknown) => void) => {
    if (settled) {
      return;
    }

    settled = true;
    get().handleBatchStopped(sessionId);
    cleanup(false);
    reject(new Error("Transcription stopped."));
  };

  await new Promise<void>((resolve, reject) => {
    transcriptionEvents.transcriptionEvent
      .listen(({ payload }) => {
        if (settled || payload.session_id !== sessionId) {
          return;
        }

        if (payload.type === "started") {
          return;
        }

        if (payload.type === "progress") {
          stopSyntheticProgress();
          get().handleBatchResponseStreamed(sessionId, payload.event);
          return;
        }

        if (payload.type === "completed") {
          resolveSuccess(
            {
              response: payload.response,
            },
            resolve,
            reject,
          );
          return;
        }

        if (payload.type === "stopped") {
          rejectStopped(reject);
          return;
        }

        if (payload.type === "failed") {
          rejectFailure(payload.error, reject, {
            terminalReason:
              payload.code === "timed_out" ? "timed_out" : "failed",
            errorCode: payload.code,
          });
        }
      })
      .then((fn) => {
        unlisten = fn;
        if (options?.signal?.aborted) {
          rejectStopped(reject);
          return;
        }

        const startNative = () =>
          transcriptionCommands
            .startTranscription(params)
            .then(async (result) => {
              if (options?.signal?.aborted) {
                try {
                  if (result.status === "ok")
                    await transcriptionCommands.stopTranscription(sessionId);
                } finally {
                  rejectStopped(reject);
                }
                return;
              }
              if (settled) {
                return;
              }

              if (result.status === "error") {
                const running = await findAdoptableBatchSession(
                  params,
                  result.error,
                );
                if (running) {
                  startedAt = running.started_at_ms;
                  return;
                }
                console.error(result.error);
                rejectFailure(result.error, reject);
              }
            });

        const resumeRecovered = async () => {
          const session = await findRunningBatchSession(sessionId);
          if (settled) {
            return;
          }
          if (!session || !matchesBatchParams(session, params)) {
            return startNative();
          }
          if (!session.completed) {
            startedAt = session.started_at_ms;
            return;
          }
          const response = await readCompletedBatchResponse(sessionId);
          if (settled) {
            return;
          }
          if (response) {
            resolveSuccess({ response }, resolve, reject);
            return;
          }
          return startNative();
        };

        const start = resumeFromRecovery ? resumeRecovered() : startNative();

        start.catch((error) => {
          console.error(error);
          rejectFailure(error, reject);
        });
      })
      .catch((error) => {
        console.error(error);
        rejectFailure(error, reject);
      });
  });

  if (options?.notifyOnCompletion !== false) {
    await notifyBatchCompleted(sessionId);
  }
};

async function findAdoptableBatchSession(
  params: TranscriptionParams,
  startError: string,
) {
  if (!startError.includes(SESSION_ALREADY_RUNNING_ERROR)) {
    return undefined;
  }
  const running = await findRunningBatchSession(params.session_id);
  return running && matchesBatchParams(running, params) ? running : undefined;
}

export async function hasConflictingBatchSession(params: TranscriptionParams) {
  const running = await findRunningBatchSession(params.session_id);
  return running !== undefined && !matchesBatchParams(running, params);
}

async function readCompletedBatchResponse(sessionId: string) {
  const result =
    await transcriptionCommands.getCompletedTranscription(sessionId);
  if (result.status === "error") {
    console.error("[runBatch] failed to read completed batch", result.error);
    return undefined;
  }
  return result.data?.response;
}

export async function acknowledgeCompletedBatch(sessionId: string) {
  try {
    const result =
      await transcriptionCommands.acknowledgeCompletedTranscription(sessionId);
    if (result.status === "error") {
      console.error("[runBatch] failed to acknowledge batch", result.error);
    }
  } catch (error) {
    console.error("[runBatch] failed to acknowledge batch", error);
  }
}

async function findRunningBatchSession(sessionId: string) {
  try {
    const result = await transcriptionCommands.listTranscriptionSessions();
    if (result.status === "error") {
      console.error("[runBatch] failed to list batch sessions", result.error);
      return undefined;
    }
    return result.data.find((session) => session.session_id === sessionId);
  } catch (error) {
    console.error("[runBatch] failed to list batch sessions", error);
    return undefined;
  }
}

function matchesBatchParams(
  session: TranscriptionSession,
  params: TranscriptionParams,
) {
  return (
    session.file_path === params.file_path &&
    session.provider === params.provider &&
    (session.model ?? null) === (params.model ?? null)
  );
}

export async function recoverRunningBatchSessions<T extends BatchStore>(
  get: StoreApi<T>["getState"],
  onResumable?: (sessions: TranscriptionSession[]) => void,
) {
  const pending = new Set<string>();
  const resumable = new Set<string>();
  const finished = new Map<string, TranscriptionEvent["type"]>();
  let unlisten: (() => void) | undefined;
  const release = (sessionId: string) => {
    pending.delete(sessionId);
    if (pending.size === 0) {
      unlisten?.();
      unlisten = undefined;
    }
  };

  unlisten = await transcriptionEvents.transcriptionEvent.listen(
    ({ payload }) => {
      const sessionId = payload.session_id;
      if (!pending.has(sessionId)) {
        if (payload.type !== "started" && payload.type !== "progress") {
          finished.set(sessionId, payload.type);
        }
        return;
      }
      if (get().batch[sessionId]?.recovered !== true) {
        release(sessionId);
        return;
      }

      switch (payload.type) {
        case "started":
          return;
        case "progress":
          get().handleBatchResponseStreamed(sessionId, payload.event);
          return;
        case "completed":
          if (resumable.has(sessionId)) {
            get().handleBatchCompleted(sessionId);
          } else {
            console.warn(
              "[runBatch] recovered batch finished without a persist target",
              { sessionId },
            );
            get().clearBatchSession(sessionId);
          }
          break;
        case "stopped":
          get().handleBatchStopped(sessionId);
          break;
        case "failed":
          get().handleBatchFailed(
            sessionId,
            payload.error,
            payload.code === "timed_out" ? "timed_out" : "failed",
            payload.code,
          );
          break;
      }
      release(sessionId);
    },
  );

  try {
    const result = await transcriptionCommands.listTranscriptionSessions();
    if (result.status === "error") {
      throw new Error(result.error);
    }
    const recovered: TranscriptionSession[] = [];
    for (const session of result.data) {
      if (get().batch[session.session_id]) {
        continue;
      }
      const finishedAs = finished.get(session.session_id);
      const completed = session.completed || finishedAs === "completed";
      if (
        (finishedAs !== undefined && finishedAs !== "completed") ||
        (completed && !session.resume_context)
      ) {
        continue;
      }
      get().handleBatchRecovered(session.session_id);
      if (session.resume_context) {
        resumable.add(session.session_id);
        recovered.push(session);
      }
      if (completed) {
        get().handleBatchCompleted(session.session_id);
      } else {
        pending.add(session.session_id);
      }
    }
    if (recovered.length > 0) {
      onResumable?.(recovered);
    }
  } finally {
    finished.clear();
    if (pending.size === 0) {
      unlisten?.();
      unlisten = undefined;
    }
  }

  return () => {
    pending.clear();
    unlisten?.();
    unlisten = undefined;
  };
}

export function shouldUseSyntheticBatchProgress(params: TranscriptionParams) {
  if (params.provider === "soniqo") {
    return false;
  }

  if (params.provider === "argmax" || params.provider === "whispercpp") {
    return false;
  }

  if (params.provider === "am") {
    return !expectsAmProgressiveBatch(params);
  }

  if (params.provider === "openai") {
    return !OPENAI_PROGRESSIVE_BATCH_MODELS.has(params.model ?? "");
  }

  return true;
}

function expectsAmProgressiveBatch(params: TranscriptionParams) {
  if (isLocalArgmaxUrl(params.base_url)) {
    return true;
  }

  if (isOpenAIUrl(params.base_url)) {
    return OPENAI_PROGRESSIVE_BATCH_MODELS.has(params.model ?? "");
  }

  return false;
}

function isLocalArgmaxUrl(baseUrl: string) {
  try {
    const url = new URL(baseUrl);
    return isLocalHost(url.hostname) && !url.pathname.includes("/stt");
  } catch {
    return false;
  }
}

function isOpenAIUrl(baseUrl: string) {
  try {
    const hostname = new URL(baseUrl).hostname;
    return hostname === "openai.com" || hostname.endsWith(".openai.com");
  } catch {
    return false;
  }
}

function isLocalHost(hostname: string) {
  return (
    hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1"
  );
}

export function syntheticBatchProgress(elapsedMs: number) {
  const elapsed = Math.max(0, elapsedMs);
  const eased =
    1 - Math.exp(-elapsed / SYNTHETIC_BATCH_PROGRESS_TIME_CONSTANT_MS);
  return Math.min(
    SYNTHETIC_BATCH_PROGRESS_MAX,
    SYNTHETIC_BATCH_PROGRESS_INITIAL +
      eased * (SYNTHETIC_BATCH_PROGRESS_MAX - SYNTHETIC_BATCH_PROGRESS_INITIAL),
  );
}
