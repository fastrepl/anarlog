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
  type CaptureConfigUpdate,
  type CaptureLifecycleEvent,
  type CaptureSnapshot,
  type CaptureParams,
  type CaptureStatusEvent,
  type LiveTranscriptDelta,
  type LiveTranscriptSegmentDelta,
} from "@hypr/plugin-transcription";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  type GeneralState,
  type LiveIntervalId,
  markLiveActive,
  markLiveFinalizing,
  markLiveInactive,
  markLiveStartFailed,
  noteLiveTranscriptActivity,
  setLiveState,
  tickTranscriptionStallWatchdog,
  updateLiveAmplitude,
  updateLiveProgress,
} from "./general-shared";
import type { TranscriptActions, TranscriptState } from "./transcript";

import { getSessionResourcePath } from "~/session/resource-path";
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

export const updateLiveSessionConfig = (update: CaptureConfigUpdate) =>
  fromResult(listenerCommands.updateCaptureConfig(update));

function getAutoStopTriggerAppIds(
  appIds: string[] | null,
  bundleId: string,
): string[] {
  return [
    ...new Set(
      (appIds ?? []).filter(
        (id) => id && id !== bundleId && !id.startsWith("pid:"),
      ),
    ),
  ];
}

const clearLiveInterval = (intervalId?: LiveIntervalId) => {
  if (intervalId) {
    clearInterval(intervalId);
  }
};

const notifyTranscriptionStalled = () => {
  sonnerToast.warning("Live transcription stalled", {
    id: "live-transcription-stalled",
    description:
      "Anarlog keeps recording. The missing part of the transcript will be rebuilt from the recording when you stop listening.",
  });
};

const createLiveSecondsInterval = <T extends GeneralState>(
  set: StoreApi<T>["setState"],
  guard?: (live: GeneralState["live"]) => boolean,
): LiveIntervalId =>
  setInterval(() => {
    let stalled = false;
    setLiveState(set, (live) => {
      if (guard && !guard(live)) {
        return;
      }
      live.seconds += 1;
      stalled = tickTranscriptionStallWatchdog(live);
    });
    if (stalled) {
      notifyTranscriptionStalled();
    }
  }, 1000);

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
          live.needsBatchRepair ||=
            payload.requested_live_transcription &&
            (!payload.live_transcription_active || payload.degraded !== null);
        });
        return;
      }

      clearLiveInterval(currentLive.intervalId);

      const intervalId = createLiveSecondsInterval(set);

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
      currentLive.sessionId === targetSessionId
        ? currentLive.seconds
        : (currentLive.finalizingBySession[targetSessionId]?.seconds ?? 0);
    const onStopped = get().takeOnStopped(targetSessionId);
    const unlisteners = currentLive.eventUnlistenersBySession[targetSessionId];
    const hasUnfinalizedTranscript =
      currentLive.sessionId === targetSessionId &&
      Object.values(get().partialWordsByChannel).some(
        (words) => words.length > 0,
      );
    const needsBatchRepair =
      currentLive.sessionId === targetSessionId
        ? currentLive.needsBatchRepair ||
          (payload.requested_live_transcription && hasUnfinalizedTranscript)
        : (currentLive.finalizingBySession[targetSessionId]?.needsBatchRepair ??
          false);

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
        needsBatchRepair,
      });
    }
  },
  progress: (payload) => {
    if (payload.session_id !== targetSessionId) {
      return;
    }

    if (get().live.sessionId !== targetSessionId) {
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
      if (get().live.sessionId !== targetSessionId) {
        return;
      }

      setLiveState(set, (live) => {
        updateLiveAmplitude(live, payload.mic, payload.speaker);
      });
      return;
    }

    if (payload.type === "transcript_delta") {
      const delta = payload.delta as unknown as LiveTranscriptDelta;
      if (
        get().live.sessionId === targetSessionId &&
        (delta.new_words.length > 0 || delta.partials.length > 0)
      ) {
        setLiveState(set, (live) => {
          noteLiveTranscriptActivity(live, {
            hasFinalWords: delta.new_words.length > 0,
          });
        });
      }
      get().handleTranscriptDelta(targetSessionId, delta, {
        updateLivePreview:
          get().live.sessionId === targetSessionId &&
          get().live.liveTranscriptionActive === true,
      });
      return;
    }

    if (payload.type === "transcript_segment_delta") {
      if (get().live.sessionId !== targetSessionId) {
        return;
      }

      get().handleTranscriptSegmentDelta(
        payload.delta as unknown as LiveTranscriptSegmentDelta,
      );
      return;
    }

    if (payload.type === "mic_muted") {
      if (get().live.sessionId !== targetSessionId) {
        return;
      }

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
  clearLiveEventUnlisteners(
    get().live.eventUnlistenersBySession[targetSessionId],
  );
  setLiveState(set, (live) => {
    delete live.eventUnlistenersBySession[targetSessionId];
  });

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

    const sessionPath = getSessionResourcePath(dataDirPath, targetSessionId);
    const app_meeting = micUsingApps?.[0] ?? null;
    const triggerAppIds = getAutoStopTriggerAppIds(micUsingApps, bundleId);

    if (triggerAppIds.length > 0) {
      setLiveState(set, (live) => {
        if (live.sessionId === targetSessionId) {
          live.triggerAppIds = triggerAppIds;
        }
      });
    }

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

export const attachLiveSession = <T extends LiveStore>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
  targetSessionId: string,
): Promise<void> => {
  const currentLive = get().live;
  if (currentLive.eventUnlistenersBySession[targetSessionId]) {
    return Promise.resolve();
  }

  const pendingUnlisteners: (() => void)[] = [];
  let registeredUnlisteners = pendingUnlisteners;
  setLiveState(set, (live) => {
    live.eventUnlistenersBySession[targetSessionId] = pendingUnlisteners;
    if (!live.sessionId) {
      live.sessionId = targetSessionId;
    }
  });

  const handlers = createSessionEventHandlers(set, get, targetSessionId);

  const program = Effect.gen(function* () {
    const unlisteners = yield* listenToAllSessionEvents(handlers);
    if (
      get().live.eventUnlistenersBySession[targetSessionId] !==
      pendingUnlisteners
    ) {
      clearLiveEventUnlisteners(unlisteners);
      return;
    }

    registeredUnlisteners = unlisteners;
    setLiveState(set, (live) => {
      live.eventUnlistenersBySession[targetSessionId] = unlisteners;
    });

    const snapshot = yield* fromResult(listenerCommands.getCaptureSnapshot());
    applyCaptureSnapshot(set, get, targetSessionId, snapshot);
  });

  return Effect.runPromiseExit(program).then((exit) =>
    Exit.match(exit, {
      onFailure: (cause) => {
        console.error("[listener] failed to attach live session:", cause);
        clearLiveEventUnlisteners(registeredUnlisteners);
        setLiveState(set, (live) => {
          if (
            live.eventUnlistenersBySession[targetSessionId] ===
            registeredUnlisteners
          ) {
            delete live.eventUnlistenersBySession[targetSessionId];
          }
          if (
            live.sessionId === targetSessionId &&
            live.status === "inactive"
          ) {
            live.sessionId = null;
          }
        });
      },
      onSuccess: () => undefined,
    }),
  );
};

function applyCaptureSnapshot<T extends GeneralState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
  targetSessionId: string,
  snapshot: CaptureSnapshot,
) {
  if (
    snapshot.state === "active" &&
    snapshot.activeSessionId === targetSessionId
  ) {
    const currentLive = get().live;
    if (currentLive.sessionId !== targetSessionId) {
      clearLiveInterval(currentLive.intervalId);
    }

    const intervalId =
      currentLive.sessionId === targetSessionId && currentLive.intervalId
        ? currentLive.intervalId
        : createLiveSecondsInterval(
            set,
            (live) =>
              live.sessionId === targetSessionId && live.status === "active",
          );

    setLiveState(set, (live) => {
      markLiveActive(
        live,
        targetSessionId,
        intervalId,
        snapshot.requestedLiveTranscription ?? true,
        snapshot.liveTranscriptionActive ?? true,
        null,
      );
    });
    return;
  }

  if (
    snapshot.state === "finalizing" &&
    snapshot.finalizingSessionIds.includes(targetSessionId)
  ) {
    setLiveState(set, (live) => {
      if (!live.sessionId) {
        live.sessionId = targetSessionId;
      }
      markLiveFinalizing(live, targetSessionId);
    });
    return;
  }

  setLiveState(set, (live) => {
    if (live.sessionId === targetSessionId && live.status === "inactive") {
      live.sessionId = null;
    }
  });
}

export const stopLiveSession = <T extends GeneralState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
) => {
  const sessionId = get().live.sessionId;

  if (sessionId) {
    setLiveState(set, (live) => {
      if (live.sessionId !== sessionId || live.status !== "active") {
        return;
      }

      clearLiveInterval(live.intervalId);
      markLiveFinalizing(live, sessionId);
    });
  }

  const program = Effect.gen(function* () {
    yield* stopSessionEffect();
  });

  void Effect.runPromiseExit(program).then((exit) => {
    Exit.match(exit, {
      onFailure: (cause) => {
        console.error("Failed to stop session:", cause);
        setLiveState(set, (live) => {
          if (sessionId && live.sessionId === sessionId) {
            delete live.finalizingBySession[sessionId];
            if (live.status === "finalizing") {
              const intervalId = createLiveSecondsInterval(
                set,
                (currentLive) =>
                  currentLive.sessionId === sessionId &&
                  currentLive.status === "active",
              );
              live.status = "active";
              live.intervalId = intervalId;
            }
          }
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
            const sessionPath = getSessionResourcePath(dataDirPath, sessionId);
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
