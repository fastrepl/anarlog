import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import {
  commands as listenerCommands,
  type SessionParams,
} from "@hypr/plugin-listener";
import type { BatchParams } from "@hypr/plugin-listener2";

import type { BatchActions, BatchState } from "./batch";
import { runBatchSession } from "./general-batch";
import { startLiveSession, stopLiveSession } from "./general-live";
import {
  type GeneralState,
  type SessionMode,
  initialGeneralState,
  markLiveStartRequested,
  setLiveState,
} from "./general-shared";
import type { HandlePersistCallback, TranscriptActions } from "./transcript";

export type { GeneralState, SessionMode } from "./general-shared";

export type GeneralActions = {
  start: (
    params: SessionParams,
    options?: { handlePersist?: HandlePersistCallback },
  ) => Promise<boolean>;
  stop: () => void;
  setMuted: (value: boolean) => void;
  runBatch: (
    params: BatchParams,
    options?: { handlePersist?: HandlePersistCallback },
  ) => Promise<void>;
  getSessionMode: (sessionId: string) => SessionMode;
};

export const createGeneralSlice = <
  T extends GeneralState &
    GeneralActions &
    TranscriptActions &
    BatchActions &
    BatchState,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): GeneralState & GeneralActions => ({
  ...initialGeneralState,
  start: async (params: SessionParams, options) => {
    const targetSessionId = params.session_id;

    if (!targetSessionId) {
      console.error("[listener] 'start' requires a session_id");
      return false;
    }

    const currentMode = get().getSessionMode(targetSessionId);
    if (currentMode === "running_batch") {
      console.warn(
        `[listener] cannot start live session while batch processing session ${targetSessionId}`,
      );
      return false;
    }

    const currentLive = get().live;
    if (currentLive.loading || currentLive.status !== "inactive") {
      console.warn(
        "[listener] cannot start live session while another session is running",
      );
      return false;
    }

    setLiveState(set, (live) => {
      markLiveStartRequested(
        live,
        targetSessionId,
        params.transcription_mode,
        params.recording_mode,
      );
    });

    if (options?.handlePersist) {
      get().setTranscriptPersist(options.handlePersist);
    }

    const started = await startLiveSession(set, get, targetSessionId, params);
    if (!started && options?.handlePersist) {
      get().setTranscriptPersist(undefined);
    }

    return started;
  },
  stop: () => {
    stopLiveSession(set, get);
  },
  setMuted: (value) => {
    set((state) =>
      mutate(state, (draft) => {
        draft.live.muted = value;
        void listenerCommands.setMicMuted(value);
      }),
    );
  },
  runBatch: async (params, options) => {
    const sessionId = params.session_id;

    if (!sessionId) {
      console.error("[listener] 'runBatch' requires params.session_id");
      return;
    }

    const mode = get().getSessionMode(sessionId);
    if (mode === "active" || mode === "finalizing") {
      console.warn(
        `[listener] cannot start batch processing while session ${sessionId} is live`,
      );
      return;
    }

    if (mode === "running_batch") {
      console.warn(
        `[listener] session ${sessionId} is already processing in batch mode`,
      );
      return;
    }

    if (options?.handlePersist) {
      get().setBatchPersist(sessionId, options.handlePersist);
    }

    await runBatchSession(get, sessionId, params);
  },
  getSessionMode: (sessionId) => {
    if (!sessionId) {
      return "inactive";
    }

    const state = get();

    if (state.live.sessionId === sessionId) {
      return state.live.status;
    }

    if (state.batch[sessionId] && !state.batch[sessionId].error) {
      return "running_batch";
    }

    return "inactive";
  },
});
