import { getIdentifier } from "@tauri-apps/api/app";
import { Effect, Exit } from "effect";
import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import { commands as detectCommands } from "@hypr/plugin-detect";
import { commands as hooksCommands } from "@hypr/plugin-hooks";
import { commands as iconCommands } from "@hypr/plugin-icon";
import {
  type DegradedError,
  commands as listenerCommands,
  events as listenerEvents,
  type SessionDataEvent,
  type SessionErrorEvent,
  type SessionLifecycleEvent,
  type SessionParams,
  type SessionProgressEvent,
  type StreamResponse,
} from "@hypr/plugin-listener";
import {
  type BatchParams,
  type BatchRunOutput,
  commands as listener2Commands,
  events as listener2Events,
} from "@hypr/plugin-listener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";

import type { BatchActions, BatchState } from "./batch";
import type { HandlePersistCallback, TranscriptActions } from "./transcript";

import { buildSessionPath } from "~/store/tinybase/persister/shared/paths";
import { fromResult } from "~/stt/fromResult";

type LiveSessionStatus = "inactive" | "active" | "finalizing";
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
  };
};

export type GeneralActions = {
  start: (
    params: SessionParams,
    options?: { handlePersist?: HandlePersistCallback },
  ) => void;
  stop: () => void;
  setMuted: (value: boolean) => void;
  runBatch: (
    params: BatchParams,
    options?: { handlePersist?: HandlePersistCallback },
  ) => Promise<void>;
  getSessionMode: (sessionId: string) => SessionMode;
};

const initialState: GeneralState = {
  live: {
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
  },
};

type EventListeners = {
  lifecycle: (payload: SessionLifecycleEvent) => void;
  progress: (payload: SessionProgressEvent) => void;
  error: (payload: SessionErrorEvent) => void;
  data: (payload: SessionDataEvent) => void;
};

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
const stopSessionEffect = () => fromResult(listenerCommands.stopSession());

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
  ...initialState,
  start: (params: SessionParams, options) => {
    const targetSessionId = params.session_id;

    if (!targetSessionId) {
      console.error("[listener] 'start' requires a session_id");
      return;
    }

    const currentMode = get().getSessionMode(targetSessionId);
    if (currentMode === "running_batch") {
      console.warn(
        `[listener] cannot start live session while batch processing session ${targetSessionId}`,
      );
      return;
    }

    set((state) =>
      mutate(state, (draft) => {
        draft.live.loading = true;
        draft.live.sessionId = targetSessionId;
      }),
    );

    if (options?.handlePersist) {
      get().setTranscriptPersist(options.handlePersist);
    }

    const handleLifecycleEvent = (payload: SessionLifecycleEvent) => {
      if (payload.session_id !== targetSessionId) {
        return;
      }

      if (payload.type === "active") {
        const currentState = get();

        if (
          currentState.live.status === "active" &&
          currentState.live.intervalId
        ) {
          set((state) =>
            mutate(state, (draft) => {
              draft.live.degraded = payload.error ?? null;
            }),
          );
          return;
        }

        if (currentState.live.intervalId) {
          clearInterval(currentState.live.intervalId);
        }

        const intervalId = setInterval(() => {
          set((s) =>
            mutate(s, (d) => {
              d.live.seconds += 1;
            }),
          );
        }, 1000);

        void iconCommands.setRecordingIndicator(true);

        set((state) =>
          mutate(state, (draft) => {
            draft.live.status = "active";
            draft.live.loading = false;
            draft.live.loadingPhase = "idle";
            draft.live.seconds = 0;
            draft.live.intervalId = intervalId;
            draft.live.sessionId = targetSessionId;
            draft.live.degraded = payload.error ?? null;
          }),
        );
      } else if (payload.type === "finalizing") {
        set((state) =>
          mutate(state, (draft) => {
            if (draft.live.intervalId) {
              clearInterval(draft.live.intervalId);
              draft.live.intervalId = undefined;
            }
            draft.live.status = "finalizing";
            draft.live.loading = true;
          }),
        );
      } else if (payload.type === "inactive") {
        const currentState = get();
        if (currentState.live.eventUnlisteners) {
          currentState.live.eventUnlisteners.forEach((fn) => fn());
        }

        void iconCommands.setRecordingIndicator(false);

        set((state) =>
          mutate(state, (draft) => {
            draft.live.status = "inactive";
            draft.live.loading = false;
            draft.live.loadingPhase = "idle";
            draft.live.sessionId = null;
            draft.live.eventUnlisteners = undefined;
            draft.live.lastError = payload.error ?? null;
            draft.live.device = null;
            draft.live.degraded = null;
            draft.live.muted = initialState.live.muted;
          }),
        );

        get().resetTranscript();
      }
    };

    const handleProgressEvent = (payload: SessionProgressEvent) => {
      if (payload.session_id !== targetSessionId) {
        return;
      }

      if (payload.type === "audio_initializing") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.loadingPhase = "audio_initializing";
            draft.live.lastError = null;
          }),
        );
      } else if (payload.type === "audio_ready") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.loadingPhase = "audio_ready";
            draft.live.device = payload.device;
          }),
        );
      } else if (payload.type === "connecting") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.loadingPhase = "connecting";
          }),
        );
      } else if (payload.type === "connected") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.loadingPhase = "connected";
          }),
        );
      }
    };

    const handleErrorEvent = (payload: SessionErrorEvent) => {
      if (payload.session_id !== targetSessionId) {
        return;
      }

      if (payload.type === "audio_error") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.lastError = payload.error;
            if (payload.is_fatal) {
              draft.live.loading = false;
            }
          }),
        );
      } else if (payload.type === "connection_error") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.lastError = payload.error;
          }),
        );
      }
    };

    const handleDataEvent = (payload: SessionDataEvent) => {
      if (payload.session_id !== targetSessionId) {
        return;
      }

      if (payload.type === "audio_amplitude") {
        set((state) =>
          mutate(state, (draft) => {
            const mic = Math.max(0, Math.min(1, payload.mic / 1000));
            const speaker = Math.max(0, Math.min(1, payload.speaker / 1000));
            draft.live.amplitude = {
              mic,
              speaker,
            };
          }),
        );
      } else if (payload.type === "stream_response") {
        const response = payload.response;
        get().handleTranscriptResponse(response as unknown as StreamResponse);
      } else if (payload.type === "mic_muted") {
        set((state) =>
          mutate(state, (draft) => {
            draft.live.muted = payload.value;
          }),
        );
      }
    };

    const program = Effect.gen(function* () {
      const unlisteners = yield* listenToAllSessionEvents({
        lifecycle: handleLifecycleEvent,
        progress: handleProgressEvent,
        error: handleErrorEvent,
        data: handleDataEvent,
      });

      set((state) =>
        mutate(state, (draft) => {
          draft.live.eventUnlisteners = unlisteners;
        }),
      );

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
      set((state) =>
        mutate(state, (draft) => {
          draft.live.status = "active";
          draft.live.loading = false;
          draft.live.sessionId = targetSessionId;
        }),
      );
    });

    void Effect.runPromiseExit(program).then((exit) => {
      Exit.match(exit, {
        onFailure: (cause) => {
          console.error(JSON.stringify(cause));
          set((state) =>
            mutate(state, (draft) => {
              if (draft.live.intervalId) {
                clearInterval(draft.live.intervalId);
                draft.live.intervalId = undefined;
              }

              if (draft.live.eventUnlisteners) {
                draft.live.eventUnlisteners.forEach((fn) => fn());
              }
              draft.live.eventUnlisteners = undefined;
              draft.live.loading = false;
              draft.live.loadingPhase = "idle";
              draft.live.status = "inactive";
              draft.live.amplitude = { mic: 0, speaker: 0 };
              draft.live.seconds = 0;
              draft.live.sessionId = null;
              draft.live.muted = initialState.live.muted;
              draft.live.lastError = null;
              draft.live.device = null;
              draft.live.degraded = null;
            }),
          );
        },
        onSuccess: () => {},
      });
    });
  },
  stop: () => {
    const sessionId = get().live.sessionId;

    const program = Effect.gen(function* () {
      yield* stopSessionEffect();
    });

    void Effect.runPromiseExit(program).then((exit) => {
      Exit.match(exit, {
        onFailure: (cause) => {
          console.error("Failed to stop session:", cause);
          set((state) =>
            mutate(state, (draft) => {
              draft.live.loading = false;
            }),
          );
        },
        onSuccess: () => {
          if (sessionId) {
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
          }
        },
      });
    });
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

      const errorMessage =
        error instanceof Error ? error.message : String(error);
      get().handleBatchFailed(sessionId, errorMessage);
      cleanup(clearSession);
      reject(error);
    };

    await new Promise<void>((resolve, reject) => {
      listener2Events.batchEvent
        .listen(({ payload }) => {
          if (settled) {
            return;
          }

          if (payload.session_id !== sessionId) {
            return;
          }

          if (payload.type === "batchStarted") {
            get().handleBatchStarted(payload.session_id);
            return;
          }

          if (payload.type === "batchProgress") {
            get().handleBatchResponseStreamed(
              sessionId,
              payload.response,
              payload.percentage,
            );
            return;
          }

          if (payload.type === "batchFailed") {
            rejectFailure(payload.error, reject);
            return;
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
