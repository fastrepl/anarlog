import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import type {
  DegradedError,
  RecordingMode,
  SessionErrorEvent,
  SessionProgressEvent,
  TranscriptionMode,
} from "@hypr/plugin-transcription";

export type LiveSessionStatus = "inactive" | "active" | "finalizing";
export type SessionMode = LiveSessionStatus | "running_batch";

export type LoadingPhase =
  | "idle"
  | "audio_initializing"
  | "audio_ready"
  | "connecting"
  | "connected";

export type GeneralState = {
  live: {
    eventUnlisteners?: (() => void)[];
    loading: boolean;
    loadingPhase: LoadingPhase;
    status: LiveSessionStatus;
    amplitude: { mic: number; speaker: number };
    seconds: number;
    intervalId?: NodeJS.Timeout;
    sessionId: string | null;
    muted: boolean;
    lastError: string | null;
    device: string | null;
    degraded: DegradedError | null;
    requestedTranscriptionMode: TranscriptionMode | null;
    currentTranscriptionMode: TranscriptionMode | null;
    recordingMode: RecordingMode | null;
  };
};

type LiveState = GeneralState["live"];

const initialLiveState: LiveState = {
  status: "inactive",
  loading: false,
  loadingPhase: "idle",
  amplitude: { mic: 0, speaker: 0 },
  seconds: 0,
  sessionId: null,
  muted: false,
  lastError: null,
  device: null,
  degraded: null,
  requestedTranscriptionMode: null,
  currentTranscriptionMode: null,
  recordingMode: null,
};

export const initialGeneralState: GeneralState = {
  live: initialLiveState,
};

export const setLiveState = <T extends GeneralState>(
  set: StoreApi<T>["setState"],
  update: (live: LiveState) => void,
) => {
  set((state) =>
    mutate(state, (draft) => {
      update(draft.live);
    }),
  );
};

export const markLiveStartRequested = (
  live: LiveState,
  sessionId: string,
  requestedTranscriptionMode: TranscriptionMode,
  recordingMode: RecordingMode,
) => {
  live.loading = true;
  live.sessionId = sessionId;
  live.requestedTranscriptionMode = requestedTranscriptionMode;
  live.currentTranscriptionMode = requestedTranscriptionMode;
  live.recordingMode = recordingMode;
};

export const markLiveActive = (
  live: LiveState,
  sessionId: string,
  intervalId: NodeJS.Timeout,
  requestedTranscriptionMode: TranscriptionMode,
  transcriptionMode: TranscriptionMode,
  degraded: DegradedError | null,
) => {
  live.status = "active";
  live.loading = false;
  live.loadingPhase = "idle";
  live.seconds = 0;
  live.intervalId = intervalId;
  live.sessionId = sessionId;
  live.degraded = degraded;
  live.requestedTranscriptionMode = requestedTranscriptionMode;
  live.currentTranscriptionMode = transcriptionMode;
};

export const markLiveFinalizing = (live: LiveState) => {
  live.status = "finalizing";
  live.loading = true;
  live.intervalId = undefined;
};

export const markLiveInactive = (live: LiveState, error: string | null) => {
  live.status = "inactive";
  live.loading = false;
  live.loadingPhase = "idle";
  live.sessionId = null;
  live.eventUnlisteners = undefined;
  live.intervalId = undefined;
  live.lastError = error;
  live.device = null;
  live.degraded = null;
  live.requestedTranscriptionMode = null;
  live.currentTranscriptionMode = null;
  live.recordingMode = null;
  live.muted = initialLiveState.muted;
};

export const markLiveStartFailed = (live: LiveState) => {
  live.eventUnlisteners = undefined;
  live.intervalId = undefined;
  live.loading = false;
  live.loadingPhase = "idle";
  live.status = "inactive";
  live.amplitude = { mic: 0, speaker: 0 };
  live.seconds = 0;
  live.sessionId = null;
  live.muted = initialLiveState.muted;
  live.lastError = null;
  live.device = null;
  live.degraded = null;
  live.requestedTranscriptionMode = null;
  live.currentTranscriptionMode = null;
  live.recordingMode = null;
};

export const updateLiveProgress = (
  live: LiveState,
  payload: SessionProgressEvent,
) => {
  switch (payload.type) {
    case "audio_initializing":
      live.loadingPhase = "audio_initializing";
      live.lastError = null;
      return;
    case "audio_ready":
      live.loadingPhase = "audio_ready";
      live.device = payload.device;
      return;
    case "connecting":
      live.loadingPhase = "connecting";
      return;
    case "connected":
      live.loadingPhase = "connected";
      return;
  }
};

export const updateLiveError = (
  live: LiveState,
  payload: SessionErrorEvent,
) => {
  switch (payload.type) {
    case "audio_error":
      live.lastError = payload.error;
      if (payload.is_fatal) {
        live.loading = false;
      }
      return;
    case "connection_error":
      live.lastError = payload.error;
      return;
  }
};

export const updateLiveAmplitude = (
  live: LiveState,
  mic: number,
  speaker: number,
) => {
  live.amplitude = {
    mic: Math.max(0, Math.min(1, mic / 1000)),
    speaker: Math.max(0, Math.min(1, speaker / 1000)),
  };
};
