import type { StoreApi } from "zustand";

import {
  type BatchErrorCode,
  type TranscriptionParams,
  commands as transcriptionCommands,
  events as transcriptionEvents,
} from "@hypr/plugin-transcription";

import type { BatchActions, BatchState } from "./batch";

type BatchStore = BatchActions & BatchState;

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
  ) => {
    if (settled) {
      return;
    }

    settled = true;

    try {
      get().handleBatchResponse(sessionId, output.response);
      cleanup();
    } catch (error) {
      console.error("[runBatch] error handling batch response", error);
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      get().handleBatchFailed(sessionId, errorMessage);
      cleanup(false);
      throw error;
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
};
