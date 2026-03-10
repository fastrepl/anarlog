import { getIdentifier } from "@tauri-apps/api/app";
import { Effect, Exit } from "effect";
import type { StoreApi } from "zustand";

import { commands as detectCommands } from "@hypr/plugin-detect";
import { commands as hooksCommands } from "@hypr/plugin-hooks";
import { commands as iconCommands } from "@hypr/plugin-icon";
import {
  commands as listenerCommands,
  events as listenerEvents,
  type SessionDataEvent,
  type SessionErrorEvent,
  type SessionLifecycleEvent,
  type SessionParams,
  type SessionProgressEvent,
  type StopSessionParams,
  type StreamResponse,
} from "@hypr/plugin-listener";
import { commands as settingsCommands } from "@hypr/plugin-settings";

import {
  type GeneralState,
  markLiveActive,
  markLiveFinalizing,
  markLiveInactive,
  markLiveStartFailed,
  setLiveState,
  updateLiveAmplitude,
  updateLiveError,
  updateLiveProgress,
} from "./general-shared";
import type { TranscriptActions } from "./transcript";

import { buildSessionPath } from "~/store/tinybase/persister/shared/paths";
import { fromResult } from "~/stt/fromResult";

type EventListeners = {
  lifecycle: (payload: SessionLifecycleEvent) => void;
  progress: (payload: SessionProgressEvent) => void;
  error: (payload: SessionErrorEvent) => void;
  data: (payload: SessionDataEvent) => void;
};

type LiveStore = GeneralState & TranscriptActions;

const listenToAllSessionEvents = (
  handlers: EventListeners,
): Effect.Effect<(() => void)[], unknown> =>
  Effect.tryPromise({
    try: async () => {
      const unlisteners = await Promise.all([
        listenerEvents.sessionLifecycleEvent.listen(({ payload }) =>
          handlers.lifecycle(payload),
        ),
        listenerEvents.sessionProgressEvent.listen(({ payload }) =>
          handlers.progress(payload),
        ),
        listenerEvents.sessionErrorEvent.listen(({ payload }) =>
          handlers.error(payload),
        ),
        listenerEvents.sessionDataEvent.listen(({ payload }) =>
          handlers.data(payload),
        ),
      ]);
      return unlisteners;
    },
    catch: (error) => error,
  });

const startSessionEffect = (params: SessionParams) =>
  fromResult(listenerCommands.startSession(params));

const stopSessionEffect = (params?: StopSessionParams) =>
  fromResult(listenerCommands.stopSession(params ?? null));

const clearLiveInterval = (intervalId?: NodeJS.Timeout) => {
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

    if (payload.type === "active") {
      const currentLive = get().live;

      if (currentLive.status === "active" && currentLive.intervalId) {
        setLiveState(set, (live) => {
          live.degraded = payload.error ?? null;
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
          payload.error ?? null,
        );
      });
      return;
    }

    if (payload.type === "finalizing") {
      clearLiveInterval(get().live.intervalId);
      setLiveState(set, (live) => {
        markLiveFinalizing(live);
      });
      return;
    }

    clearLiveEventUnlisteners(get().live.eventUnlisteners);
    clearLiveInterval(get().live.intervalId);

    void iconCommands.setRecordingIndicator(false);

    setLiveState(set, (live) => {
      markLiveInactive(live, payload.error ?? null);
    });

    get().resetTranscript();
  },
  progress: (payload) => {
    if (payload.session_id !== targetSessionId) {
      return;
    }

    setLiveState(set, (live) => {
      updateLiveProgress(live, payload);
    });
  },
  error: (payload) => {
    if (payload.session_id !== targetSessionId) {
      return;
    }

    setLiveState(set, (live) => {
      updateLiveError(live, payload);
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

    if (payload.type === "stream_response") {
      get().handleTranscriptResponse(
        payload.response as unknown as StreamResponse,
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
  params: SessionParams,
) => {
  const handlers = createSessionEventHandlers(set, get, targetSessionId);

  const program = Effect.gen(function* () {
    const unlisteners = yield* listenToAllSessionEvents(handlers);

    setLiveState(set, (live) => {
      live.eventUnlisteners = unlisteners;
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

  void Effect.runPromiseExit(program).then((exit) => {
    Exit.match(exit, {
      onFailure: (cause) => {
        console.error(JSON.stringify(cause));
        const currentLive = get().live;
        clearLiveInterval(currentLive.intervalId);
        clearLiveEventUnlisteners(currentLive.eventUnlisteners);
        setLiveState(set, (live) => {
          markLiveStartFailed(live);
        });
      },
      onSuccess: () => {},
    });
  });
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
