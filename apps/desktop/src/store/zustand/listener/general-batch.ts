import { getCurrentWindow } from "@tauri-apps/api/window";
import type { StoreApi } from "zustand";

import { commands as notificationCommands } from "@hypr/plugin-notification";
import {
  type BatchErrorCode,
  type TranscriptionParams,
  commands as transcriptionCommands,
  events as transcriptionEvents,
} from "@hypr/plugin-transcription";

import {
  EMPTY_BATCH_TRANSCRIPT_ERROR,
  type BatchActions,
  type BatchState,
} from "./batch";

import { createBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";

type BatchStore = BatchActions & BatchState;

async function shouldNotifyBatchCompleted() {
  try {
    const window = getCurrentWindow();
    const [focused, visible] = await Promise.all([
      window.isFocused(),
      window.isVisible(),
    ]);

    return !focused || !visible;
  } catch (error) {
    console.error("[runBatch] failed to inspect window state", error);
    return true;
  }
}

export async function showBatchCompletedNotification(
  sessionId: string,
  options?: { force?: boolean },
) {
  if (!options?.force && !(await shouldNotifyBatchCompleted())) {
    return;
  }

  try {
    const result = await notificationCommands.showNotification({
      key: createBatchCompletedNotificationKey(sessionId),
      title: "Transcription complete",
      message: "Your transcript is ready.",
      timeout: null,
      source: { type: "session", session_id: sessionId },
      start_time: null,
      participants: null,
      event_details: null,
      action_label: "Open Anarlog",
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

export const runBatchSession = async <T extends BatchStore>(
  get: StoreApi<T>["getState"],
  sessionId: string,
  params: TranscriptionParams,
) => {
  get().handleBatchStarted(sessionId);

  let unlisten: (() => void) | undefined;
  let settled = false;

  const cleanup = (clearSession = true) => {
    if (unlisten) {
      unlisten();
      unlisten = undefined;
    }

    get().clearBatchPersist(sessionId);

    if (clearSession) {
      get().clearBatchSession(sessionId);
    }
  };

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

    try {
      const handled = get().handleBatchResponse(sessionId, output.response);
      if (handled === false) {
        throw new Error(EMPTY_BATCH_TRANSCRIPT_ERROR);
      }
      cleanup();
    } catch (error) {
      console.error("[runBatch] error handling batch response", error);
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      get().handleBatchFailed(sessionId, errorMessage);
      cleanup(false);
      reject(error);
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
          get().handleBatchStarted(payload.session_id);
          return;
        }

        if (payload.type === "progress") {
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

        transcriptionCommands
          .startTranscription(params)
          .then((result) => {
            if (settled) {
              return;
            }

            if (result.status === "error") {
              console.error(result.error);
              rejectFailure(result.error, reject);
            }
          })
          .catch((error) => {
            console.error(error);
            rejectFailure(error, reject);
          });
      })
      .catch((error) => {
        console.error(error);
        rejectFailure(error, reject);
      });
  });

  await showBatchCompletedNotification(sessionId);
};
