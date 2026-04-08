import { getIdentifier } from "@tauri-apps/api/app";
import { Effect, Exit } from "effect";
import type { StoreApi } from "zustand";

import { commands as detectCommands } from "@hypr/plugin-detect";
import { commands as hooksCommands } from "@hypr/plugin-hooks";
import { commands as iconCommands } from "@hypr/plugin-icon";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import {
  commands as listenerCommands,
  events as listenerEvents,
  type CaptureDataEvent,
  type CaptureLifecycleEvent,
  type CaptureParams,
  type CaptureStatusEvent,
  type LiveTranscriptDelta,
  type LiveTranscriptSegmentDelta,
} from "@hypr/plugin-transcription";

import {
  type GeneralState,
  type LiveIntervalId,
  markLiveActive,
  markLiveFinalizing,
  markLiveInactive,
  markLiveStartFailed,
  setLiveState,
  updateLiveAmplitude,
  updateLiveProgress,
} from "./general-shared";
import type { TranscriptActions, TranscriptState } from "./transcript";

import { buildSessionPath } from "~/store/tinybase/persister/shared/paths";
import { fromResult } from "~/stt/fromResult";

type EventListeners = {
  lifecycle: (payload: CaptureLifecycleEvent) => void;
  progress: (payload: CaptureStatusEvent) => void;
  data: (payload: CaptureDataEvent) => void;
};

type LiveStore = GeneralState & TranscriptState & TranscriptActions;

const listenToAllSessionEvents = (
  handlers: EventListeners,
): Effect.Effect<(() => void)[], unknown> =>
  Effect.tryPromise({
    try: async () => {
      const unlisteners = await Promise.all([
        listenerEvents.captureLifecycleEvent.listen(({ payload }) =>
          handlers.lifecycle(payload),
        ),
        listenerEvents.captureStatusEvent.listen(({ payload }) =>
          handlers.progress(payload),
        ),
        listenerEvents.captureDataEvent.listen(({ payload }) =>
          handlers.data(payload),
        ),
      ]);
      return unlisteners;
    },
    catch: (error) => error,
  });

const startSessionEffect = (params: CaptureParams) =>
  fromResult(listenerCommands.startCapture(params));

const stopSessionEffect = () => fromResult(listenerCommands.stopCapture());

const clearLiveInterval = (intervalId?: LiveIntervalId) => {
  if (intervalId) {
    clearInterval(intervalId);
  }
};

const clearLiveEventUnlisteners = (unlisteners?: (() => void)[]) => {
  unlisteners?.forEach((fn) => fn());
};

const createSessionEventHandlers = <T extends LiveStore>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
  targetSessionId: string,
): EventListeners => ({
  lifecycle: (payload) => {
    if (payload.session_id !== targetSessionId) {
      return;
    }

    if (payload.type === "started") {
      const currentLive = get().live;

      if (currentLive.status === "active" && currentLive.intervalId) {
        setLiveState(set, (live) => {
          live.degraded = payload.degraded ?? null;
          live.requestedLiveTranscription =
            payload.requested_live_transcription;
          live.liveTranscriptionActive = payload.live_transcription_active;
        });
        return;
      }

      clearLiveInterval(currentLive.intervalId);

      const intervalId = setInterval(() => {
        setLiveState(set, (live) => {
          live.seconds += 1;
        });
      }, 1000);

      void iconCommands.setRecordingIndicator(true);

      setLiveState(set, (live) => {
        markLiveActive(
          live,
          targetSessionId,
          intervalId,
          payload.requested_live_transcription,
          payload.live_transcription_active,
          payload.degraded ?? null,
        );
      });
      return;
    }

    if (payload.type === "finalizing") {
      setLiveState(set, (live) => {
        if (live.sessionId === targetSessionId) {
          clearLiveInterval(live.intervalId);
        }
        markLiveFinalizing(live, targetSessionId);
      });
      return;
    }

    const currentLive = get().live;
    const stoppedSeconds =
      currentLive.sessionId === targetSessionId ? currentLive.seconds : 0;
    const onStopped = get().takeOnStopped(targetSessionId);
    const unlisteners = currentLive.eventUnlistenersBySession[targetSessionId];

    clearLiveEventUnlisteners(unlisteners);

    setLiveState(set, (live) => {
      delete live.eventUnlistenersBySession[targetSessionId];
      delete live.finalizingBySession[targetSessionId];

      if (live.sessionId === targetSessionId) {
        clearLiveInterval(live.intervalId);
        markLiveInactive(live, payload.error ?? null);
      }
    });

    if (currentLive.sessionId === targetSessionId) {
      void iconCommands.setRecordingIndicator(false);
      get().resetTranscript();
    }

    if (onStopped) {
      onStopped(targetSessionId, {
        durationSeconds: stoppedSeconds,
        audioPath: payload.audio_path ?? null,
        requestedLiveTranscription: payload.requested_live_transcription,
        liveTranscriptionActive: payload.live_transcription_active,
      });
    }
  },
  progress: (payload) => {
    if (payload.session_id !== targetSessionId) {
      return;
    }

    setLiveState(set, (live) => {
      updateLiveProgress(live, payload);
    });
  },
  data: (payload) => {
    if (payload.session_id !== targetSessionId) {
      return;
    }

    if (payload.type === "audio_amplitude") {
      setLiveState(set, (live) => {
        updateLiveAmplitude(live, payload.mic, payload.speaker);
      });
      return;
    }

    if (payload.type === "transcript_delta") {
      get().handleTranscriptDelta(
        targetSessionId,
        payload.delta as unknown as LiveTranscriptDelta,
      );
      return;
    }

    if (payload.type === "transcript_segment_delta") {
      get().handleTranscriptSegmentDelta(
        payload.delta as unknown as LiveTranscriptSegmentDelta,
      );
      return;
    }

    if (payload.type === "mic_muted") {
      setLiveState(set, (live) => {
        live.muted = payload.value;
      });
    }
  },
});

export const startLiveSession = <T extends LiveStore>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
  targetSessionId: string,
  params: CaptureParams,
): Promise<boolean> => {
  const handlers = createSessionEventHandlers(set, get, targetSessionId);

  const program = Effect.gen(function* () {
    const unlisteners = yield* listenToAllSessionEvents(handlers);

    setLiveState(set, (live) => {
      live.eventUnlistenersBySession[targetSessionId] = unlisteners;
    });

    const [dataDirPath, micUsingApps, bundleId] = yield* Effect.tryPromise({
      try: () =>
        Promise.all([
          settingsCommands.vaultBase().then((r) => {
            if (r.status === "error") throw new Error(r.error);
            return r.data;
          }),
          detectCommands
            .listMicUsingApplications()
            .then((r) =>
              r.status === "ok" ? r.data.map((app) => app.id) : null,
            ),
          getIdentifier().catch(() => "com.hyprnote.stable"),
        ]),
      catch: (error) => error,
    });

    const sessionPath = buildSessionPath(dataDirPath, targetSessionId);
    const app_meeting = micUsingApps?.[0] ?? null;

    yield* Effect.tryPromise({
      try: () =>
        hooksCommands.runEventHooks({
          beforeListeningStarted: {
            args: {
              resource_dir: sessionPath,
              app_hyprnote: bundleId,
              app_meeting,
            },
          },
        }),
      catch: (error) => {
        console.error("[hooks] BeforeListeningStarted failed:", error);
        return error;
      },
    });

    yield* startSessionEffect(params);

    setLiveState(set, (live) => {
      live.status = "active";
      live.loading = false;
      live.sessionId = targetSessionId;
    });
  });

  return Effect.runPromiseExit(program).then((exit) =>
    Exit.match(exit, {
      onFailure: (cause) => {
        console.error(JSON.stringify(cause));
        const currentLive = get().live;
        clearLiveInterval(currentLive.intervalId);
        clearLiveEventUnlisteners(
          currentLive.eventUnlistenersBySession[targetSessionId],
        );
        setLiveState(set, (live) => {
          delete live.eventUnlistenersBySession[targetSessionId];
          markLiveStartFailed(live);
        });
        return false;
      },
      onSuccess: () => true,
    }),
  );
};

export const stopLiveSession = <T extends GeneralState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
) => {
  const sessionId = get().live.sessionId;

  const program = Effect.gen(function* () {
    yield* stopSessionEffect();
  });

  void Effect.runPromiseExit(program).then((exit) => {
    Exit.match(exit, {
      onFailure: (cause) => {
        console.error("Failed to stop session:", cause);
        setLiveState(set, (live) => {
          live.loading = false;
        });
      },
      onSuccess: () => {
        if (!sessionId) {
          return;
        }

        void Promise.all([
          settingsCommands.vaultBase().then((r) => {
            if (r.status === "error") throw new Error(r.error);
            return r.data;
          }),
          getIdentifier().catch(() => "com.hyprnote.stable"),
        ])
          .then(([dataDirPath, bundleId]) => {
            const sessionPath = buildSessionPath(dataDirPath, sessionId);
            return hooksCommands.runEventHooks({
              afterListeningStopped: {
                args: {
                  resource_dir: sessionPath,
                  app_hyprnote: bundleId,
                  app_meeting: null,
                },
              },
            });
          })
          .catch((error) => {
            console.error("[hooks] AfterListeningStopped failed:", error);
          });
      },
    });
  });
};
