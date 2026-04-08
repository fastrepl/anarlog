import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import type {
  DegradedError,
  CaptureStatusEvent,
} from "@hypr/plugin-transcription";

export type LiveSessionStatus = "inactive" | "active" | "finalizing";
export type SessionMode = LiveSessionStatus | "running_batch";
export type LiveCaptureUiMode = "live" | "record_only" | "fallback_record_only";

export type LoadingPhase =
  | "idle"
  | "audio_initializing"
  | "audio_ready"
  | "connecting"
  | "connected";

export type LiveIntervalId = ReturnType<typeof setInterval>;

export type GeneralState = {
  live: {
    eventUnlistenersBySession: Record<string, (() => void)[]>;
    loading: boolean;
    loadingPhase: LoadingPhase;
    status: LiveSessionStatus;
    amplitude: { mic: number; speaker: number };
    seconds: number;
    intervalId?: LiveIntervalId;
    sessionId: string | null;
    muted: boolean;
    lastError: string | null;
    device: string | null;
    degraded: DegradedError | null;
    requestedLiveTranscription: boolean | null;
    liveTranscriptionActive: boolean | null;
    finalizingBySession: Record<string, { startedAtMs: number }>;
  };
};

type LiveState = GeneralState["live"];

const initialLiveState: LiveState = {
  status: "inactive",
  eventUnlistenersBySession: {},
  loading: false,
  loadingPhase: "idle",
  amplitude: { mic: 0, speaker: 0 },
  seconds: 0,
  sessionId: null,
  muted: false,
  lastError: null,
  device: null,
  degraded: null,
  requestedLiveTranscription: null,
  liveTranscriptionActive: null,
  finalizingBySession: {},
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

export const markLiveStartRequested = (live: LiveState, sessionId: string) => {
  live.loading = true;
  live.status = "inactive";
  live.sessionId = sessionId;
  live.requestedLiveTranscription = null;
  live.liveTranscriptionActive = null;
};

export const markLiveActive = (
  live: LiveState,
  sessionId: string,
  intervalId: LiveIntervalId,
  requestedLiveTranscription: boolean,
  liveTranscriptionActive: boolean,
  degraded: DegradedError | null,
) => {
  live.status = "active";
  live.loading = false;
  live.loadingPhase = "idle";
  live.seconds = 0;
  live.intervalId = intervalId;
  live.sessionId = sessionId;
  live.degraded = degraded;
  live.requestedLiveTranscription = requestedLiveTranscription;
  live.liveTranscriptionActive = liveTranscriptionActive;
};

export const markLiveFinalizing = (live: LiveState, sessionId: string) => {
  if (live.sessionId === sessionId) {
    live.status = "finalizing";
    live.loading = true;
    live.intervalId = undefined;
  }
  live.finalizingBySession[sessionId] = { startedAtMs: Date.now() };
};

export const markLiveInactive = (live: LiveState, error: string | null) => {
  live.status = "inactive";
  live.loading = false;
  live.loadingPhase = "idle";
  live.sessionId = null;
  live.intervalId = undefined;
  live.lastError = error;
  live.device = null;
  live.degraded = null;
  live.requestedLiveTranscription = null;
  live.liveTranscriptionActive = null;
  live.muted = initialLiveState.muted;
};

export const markLiveStartFailed = (live: LiveState) => {
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
  live.requestedLiveTranscription = null;
  live.liveTranscriptionActive = null;
};

export const updateLiveProgress = (
  live: LiveState,
  payload: CaptureStatusEvent,
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

export const getLiveCaptureUiMode = (
  live: Pick<
    LiveState,
    "requestedLiveTranscription" | "liveTranscriptionActive"
  >,
): LiveCaptureUiMode => {
  if (live.liveTranscriptionActive === false) {
    return live.requestedLiveTranscription === true
      ? "fallback_record_only"
      : "record_only";
  }

  return "live";
};
