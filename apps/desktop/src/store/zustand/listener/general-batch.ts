import type { StoreApi } from "zustand";

import {
  type BatchParams,
  type BatchRunOutput,
  commands as listener2Commands,
  events as listener2Events,
} from "@hypr/plugin-transcription";

import type { BatchActions, BatchState } from "./batch";

type BatchStore = BatchActions & BatchState;

export const runBatchSession = async <T extends BatchStore>(
  get: StoreApi<T>["getState"],
  sessionId: string,
  params: BatchParams,
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

  const resolveSuccess = (output: BatchRunOutput, resolve: () => void) => {
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
    clearSession = false,
  ) => {
    if (settled) {
      return;
    }

    settled = true;

    const errorMessage = error instanceof Error ? error.message : String(error);
    get().handleBatchFailed(sessionId, errorMessage);
    cleanup(clearSession);
    reject(error);
  };

  await new Promise<void>((resolve, reject) => {
    listener2Events.batchEvent
      .listen(({ payload }) => {
        if (settled || payload.session_id !== sessionId) {
          return;
        }

        if (payload.type === "batchStarted") {
          get().handleBatchStarted(payload.session_id);
          return;
        }

        if (payload.type === "batchProgress") {
          get().handleBatchResponseStreamed(sessionId, payload.event);
          return;
        }

        if (payload.type === "batchFailed") {
          rejectFailure(payload.error, reject);
        }
      })
      .then((fn) => {
        unlisten = fn;

        listener2Commands
          .runBatch(params)
          .then((result) => {
            if (settled) {
              return;
            }

            if (result.status === "error") {
              console.error(result.error);
              rejectFailure(result.error, reject);
              return;
            }

            try {
              resolveSuccess(result.data, resolve);
            } catch (error) {
              reject(error);
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
