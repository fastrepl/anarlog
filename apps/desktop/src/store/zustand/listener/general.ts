import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import {
  commands as listenerCommands,
  type CaptureParams,
} from "@hypr/plugin-transcription";
import type { TranscriptionParams } from "@hypr/plugin-transcription";

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
import type {
  BatchPersistCallback,
  LiveTranscriptPersistCallback,
  OnStoppedCallback,
  TranscriptActions,
  TranscriptState,
} from "./transcript";

export type { GeneralState, SessionMode } from "./general-shared";

export type GeneralActions = {
  start: (
    params: CaptureParams,
    options?: {
      handlePersist?: LiveTranscriptPersistCallback;
      onStopped?: OnStoppedCallback;
    },
  ) => Promise<boolean>;
  stop: () => void;
  setMuted: (value: boolean) => void;
  startTranscription: (
    params: TranscriptionParams,
    options?: { handlePersist?: BatchPersistCallback },
  ) => Promise<void>;
  stopTranscription: (sessionId: string) => Promise<void>;
  getSessionMode: (sessionId: string) => SessionMode;
};

export const createGeneralSlice = <
  T extends GeneralState &
    GeneralActions &
    TranscriptState &
    TranscriptActions &
    BatchActions &
    BatchState,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): GeneralState & GeneralActions => ({
  ...initialGeneralState,
  start: async (params: CaptureParams, options) => {
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
      markLiveStartRequested(live, targetSessionId);
    });

    if (options?.handlePersist) {
      get().setTranscriptPersist(targetSessionId, options.handlePersist);
    }
    if (options?.onStopped) {
      get().setOnStopped(targetSessionId, options.onStopped);
    }

    const started = await startLiveSession(set, get, targetSessionId, params);
    if (!started) {
      if (options?.handlePersist) {
        get().setTranscriptPersist(targetSessionId, undefined);
      }
      if (options?.onStopped) {
        get().setOnStopped(targetSessionId, undefined);
      }
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
  startTranscription: async (params, options) => {
    const sessionId = params.session_id;

    if (!sessionId) {
      throw new Error(
        "[listener] startTranscription requires params.session_id",
      );
    }

    const mode = get().getSessionMode(sessionId);
    if (mode === "active" || mode === "finalizing") {
      throw new Error(
        `[listener] cannot start batch processing while session ${sessionId} is live`,
      );
    }

    if (mode === "running_batch") {
      throw new Error(
        `[listener] session ${sessionId} is already processing in batch mode`,
      );
    }

    if (options?.handlePersist) {
      get().setBatchPersist(sessionId, options.handlePersist);
    }

    await runBatchSession(get, sessionId, params);
  },
  stopTranscription: async (sessionId) => {
    if (!sessionId) {
      return;
    }

    await listenerCommands.stopTranscription(sessionId).catch(console.error);
  },
  getSessionMode: (sessionId) => {
    if (!sessionId) {
      return "inactive";
    }

    const state = get();

    if (state.live.sessionId === sessionId) {
      return state.live.status;
    }

    if (state.live.finalizingBySession[sessionId]) {
      return "finalizing";
    }

    if (state.batch[sessionId] && !state.batch[sessionId].terminalReason) {
      return "running_batch";
    }

    return "inactive";
  },
});
