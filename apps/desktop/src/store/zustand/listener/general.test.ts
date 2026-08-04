import { create as mutate } from "mutative";
import { beforeEach, describe, expect, test, vi } from "vitest";

const {
  dispatchEventMock,
  getIdentifierMock,
  getCaptureSnapshotMock,
  listenCaptureDataMock,
  listenCaptureLifecycleMock,
  listenCaptureStatusMock,
  listMicUsingApplicationsMock,
  runEventHooksMock,
  setRecordingIndicatorMock,
  startCaptureMock,
  stopCaptureMock,
  vaultBaseMock,
} = vi.hoisted(() => ({
  dispatchEventMock: vi.fn(),
  getIdentifierMock: vi.fn(),
  getCaptureSnapshotMock: vi.fn(),
  listenCaptureDataMock: vi.fn(),
  listenCaptureLifecycleMock: vi.fn(),
  listenCaptureStatusMock: vi.fn(),
  listMicUsingApplicationsMock: vi.fn(),
  runEventHooksMock: vi.fn(),
  setRecordingIndicatorMock: vi.fn(),
  startCaptureMock: vi.fn(),
  stopCaptureMock: vi.fn(),
  vaultBaseMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: getIdentifierMock,
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: listMicUsingApplicationsMock,
  },
}));

vi.mock("@anlg/plugin-hooks", () => ({
  commands: {
    runEventHooks: runEventHooksMock,
  },
}));

vi.mock("@anlg/plugin-icon", () => ({
  commands: {
    setRecordingIndicator: setRecordingIndicatorMock,
  },
}));

vi.mock("@anlg/plugin-local-api", () => ({
  commands: {
    dispatchEvent: dispatchEventMock,
  },
}));

vi.mock("@anlg/plugin-settings", () => ({
  commands: {
    vaultBase: vaultBaseMock,
  },
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    getCaptureSnapshot: getCaptureSnapshotMock,
    setMicMuted: vi.fn(),
    startCapture: startCaptureMock,
    startTranscription: vi.fn(),
    stopCapture: stopCaptureMock,
    stopTranscription: vi.fn(),
    updateCaptureConfig: vi.fn(),
  },
  events: {
    captureDataEvent: {
      listen: listenCaptureDataMock,
    },
    captureLifecycleEvent: {
      listen: listenCaptureLifecycleMock,
    },
    captureStatusEvent: {
      listen: listenCaptureStatusMock,
    },
  },
}));

vi.mock("~/cloud-api/client", () => ({
  syncCloudApiSnapshotBestEffort: vi.fn(),
}));

import { createListenerStore } from ".";
import {
  getLiveCaptureUiMode,
  markLiveActive,
  markLiveInactive,
  markLiveStartRequested,
  updateLiveProgress,
} from "./general-shared";

import { enqueueSessionAudioOperation } from "~/session/audio-operations";

let store: ReturnType<typeof createListenerStore>;

describe("General Listener Slice", () => {
  beforeEach(() => {
    store = createListenerStore();
    vi.clearAllMocks();
    getIdentifierMock.mockResolvedValue("com.anarlog.stable");
    getCaptureSnapshotMock.mockResolvedValue({
      status: "ok",
      data: {
        activeSessionId: null,
        finalizingSessionIds: [],
        liveTranscriptionActive: null,
        requestedLiveTranscription: null,
        state: "inactive",
      },
    });
    listenCaptureDataMock.mockResolvedValue(() => {});
    listenCaptureLifecycleMock.mockResolvedValue(() => {});
    listenCaptureStatusMock.mockResolvedValue(() => {});
    listMicUsingApplicationsMock.mockResolvedValue({ status: "ok", data: [] });
    runEventHooksMock.mockResolvedValue({ status: "ok", data: null });
    setRecordingIndicatorMock.mockResolvedValue({ status: "ok", data: null });
    startCaptureMock.mockResolvedValue({ status: "ok", data: null });
    stopCaptureMock.mockResolvedValue({ status: "ok", data: null });
    dispatchEventMock.mockResolvedValue({ status: "ok", data: 1 });
    vaultBaseMock.mockResolvedValue({ status: "ok", data: "/tmp/anarlog" });
  });

  describe("Initial State", () => {
    test("initializes with correct default values", () => {
      const state = store.getState();
      expect(state.live.status).toBe("inactive");
      expect(state.live.loading).toBe(false);
      expect(state.live.amplitude).toEqual({ mic: 0, speaker: 0 });
      expect(state.live.seconds).toBe(0);
      expect(state.live.eventUnlistenersBySession).toEqual({});
      expect(state.live.intervalId).toBeUndefined();
      expect(state.live.needsBatchRepair).toBe(false);
      expect(state.live.postStopProcessingBySession).toEqual({});
      expect(state.batch).toEqual({});
    });
  });

  describe("Amplitude Updates", () => {
    test("amplitude state is initialized to zero", () => {
      const state = store.getState();
      expect(state.live.amplitude).toEqual({ mic: 0, speaker: 0 });
    });
  });

  describe("Session Mode Helpers", () => {
    test("getSessionMode defaults to inactive", () => {
      const state = store.getState();
      expect(state.getSessionMode("session-123")).toBe("inactive");
    });

    test("getSessionMode returns running_batch when session is in batch", () => {
      const sessionId = "session-456";
      const { handleBatchResponseStreamed, getSessionMode } = store.getState();

      const mockEvent = {
        type: "progress" as const,
        percentage: 0.5,
        partial_text: "test",
      };

      handleBatchResponseStreamed(sessionId, mockEvent);
      expect(getSessionMode(sessionId)).toBe("running_batch");
    });

    test("getLiveCaptureUiMode returns record_only when capture starts without live transcription", () => {
      expect(
        getLiveCaptureUiMode({
          requestedLiveTranscription: false,
          liveTranscriptionActive: false,
        }),
      ).toBe("record_only");
    });

    test("getLiveCaptureUiMode returns fallback_record_only when live transcription drops during capture", () => {
      expect(
        getLiveCaptureUiMode({
          requestedLiveTranscription: true,
          liveTranscriptionActive: false,
        }),
      ).toBe("fallback_record_only");
    });

    test("markLiveActive preserves startup progress errors", () => {
      const intervalId = setInterval(() => {}, 1000);

      store.setState((state) =>
        mutate(state, (draft) => {
          updateLiveProgress(draft.live, {
            type: "connection_error",
            session_id: "session-1",
            error: "socket closed",
          });
          markLiveActive(
            draft.live,
            "session-1",
            intervalId,
            true,
            false,
            null,
          );
        }),
      );

      clearInterval(intervalId);

      expect(store.getState().live.status).toBe("active");
      expect(store.getState().live.lastError).toBe("socket closed");
      expect(store.getState().live.lastErrorSessionId).toBe("session-1");
    });

    test("markLiveActive preserves the need for batch repair after recovery", () => {
      const intervalId = setInterval(() => {}, 1000);

      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveActive(draft.live, "session-1", intervalId, true, false, {
            type: "connection_timeout",
          });
          markLiveActive(draft.live, "session-1", intervalId, true, true, null);
        }),
      );

      clearInterval(intervalId);
      expect(store.getState().live.needsBatchRepair).toBe(true);
    });

    test("keeps same-session audio recovery after a terminal stop error", () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveStartRequested(draft.live, "session-a");
          updateLiveProgress(draft.live, {
            type: "audio_error",
            session_id: "session-a",
            error: "microphone unavailable",
            device: null,
            is_fatal: true,
          });
          markLiveInactive(
            draft.live,
            "session-a",
            "capture restart limit exceeded",
          );
        }),
      );

      expect(store.getState().live.lastError).toBe("microphone unavailable");
      expect(store.getState().live.lastErrorSessionId).toBe("session-a");
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(true);
    });

    test("clears scoped recovery after a normal stop", () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveStartRequested(draft.live, "session-a");
          updateLiveProgress(draft.live, {
            type: "audio_error",
            session_id: "session-a",
            error: "microphone unavailable",
            device: null,
            is_fatal: true,
          });
          markLiveInactive(draft.live, "session-a", null);
        }),
      );

      expect(store.getState().live.lastError).toBeNull();
      expect(store.getState().live.lastErrorSessionId).toBeNull();
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(false);
    });
  });

  describe("Batch State", () => {
    test("handleBatchResponseStreamed tracks progress per session", () => {
      const sessionId = "session-progress";
      const { handleBatchResponseStreamed, clearBatchSession } =
        store.getState();

      const mockEvent = {
        type: "segment" as const,
        percentage: 0.5,
        response: {
          type: "Results" as const,
          start: 0,
          duration: 5,
          is_final: false,
          speech_final: false,
          from_finalize: false,
          channel: {
            alternatives: [
              {
                transcript: "test",
                languages: [],
                words: [
                  {
                    word: "test",
                    punctuated_word: "test",
                    start: 0,
                    end: 0.5,
                    confidence: 0.9,
                    speaker: null,
                    language: null,
                  },
                ],
                confidence: 0.9,
              },
            ],
          },
          metadata: {
            request_id: "test-request",
            model_info: {
              name: "test-model",
              version: "1.0",
              arch: "test-arch",
            },
            model_uuid: "test-uuid",
          },
          channel_index: [0],
        },
      };

      handleBatchResponseStreamed(sessionId, mockEvent);
      expect(store.getState().batch[sessionId]).toEqual({
        percentage: 0.5,
        isComplete: false,
        phase: "transcribing",
        terminalReason: undefined,
        error: undefined,
        errorCode: undefined,
      });
      expect(
        store.getState().batchPreview[sessionId]?.wordsByChannel[0],
      ).toEqual([
        {
          text: " test",
          start_ms: 0,
          end_ms: 500,
          channel: 0,
          metadata: {
            timing: {
              source: "provider_word",
            },
          },
        },
      ]);

      clearBatchSession(sessionId);
      expect(store.getState().batch[sessionId]).toBeUndefined();
      expect(store.getState().batchPreview[sessionId]).toBeUndefined();
    });

    test("handleBatchResponseStreamed persists streamed transcript snapshots", () => {
      const sessionId = "session-streamed-persist";
      const persist = vi.fn();
      const { handleBatchResponseStreamed, setBatchPersist } = store.getState();

      setBatchPersist(sessionId, persist);

      handleBatchResponseStreamed(sessionId, {
        type: "segment",
        percentage: 0.5,
        response: {
          type: "Results",
          start: 0,
          duration: 5,
          is_final: false,
          speech_final: false,
          from_finalize: false,
          channel: {
            alternatives: [
              {
                transcript: "hello",
                languages: [],
                words: [
                  {
                    word: "hello",
                    punctuated_word: "hello",
                    start: 0,
                    end: 0.5,
                    confidence: 0.9,
                    speaker: 1,
                    language: null,
                  },
                ],
                confidence: 0.9,
              },
            ],
          },
          metadata: {
            request_id: "test-request",
            model_info: {
              name: "test-model",
              version: "1.0",
              arch: "test-arch",
            },
            model_uuid: "test-uuid",
          },
          channel_index: [0],
        },
      });

      expect(persist).toHaveBeenCalledWith(
        [
          {
            text: " hello",
            start_ms: 0,
            end_ms: 500,
            channel: 0,
            metadata: {
              timing: {
                source: "provider_word",
              },
            },
          },
        ],
        [
          {
            wordIndex: 0,
            data: {
              type: "provider_speaker_index",
              speaker_index: 1,
            },
          },
        ],
        { mode: "replace" },
      );
    });

    test("handleBatchResponse persists transcript-only batch responses", () => {
      const sessionId = "session-transcript-only-batch";
      const persist = vi.fn();
      const { handleBatchStarted, handleBatchResponse, setBatchPersist } =
        store.getState();

      handleBatchStarted(sessionId);
      setBatchPersist(sessionId, persist);

      expect(
        handleBatchResponse(sessionId, {
          metadata: { duration: 120, timing_source: "provider_word" },
          results: {
            channels: [
              {
                alternatives: [
                  {
                    transcript: "hello world",
                    confidence: 0.9,
                    words: [],
                  },
                ],
              },
            ],
          },
        }),
      ).toBe(true);

      expect(persist).toHaveBeenCalledWith(
        [
          {
            text: " hello",
            start_ms: 0,
            end_ms: 400,
            channel: 0,
            metadata: {
              timing: {
                source: "synthetic_text",
              },
            },
          },
          {
            text: " world",
            start_ms: 400,
            end_ms: 800,
            channel: 0,
            metadata: {
              timing: {
                source: "synthetic_text",
              },
            },
          },
        ],
        [],
        { mode: "replace" },
      );
      expect(store.getState().batch[sessionId]).toBeUndefined();
    });

    test("handleBatchResponseStreamed replaces preview with transcript-only result", () => {
      const sessionId = "session-transcript-only-result";
      const persist = vi.fn();
      const { handleBatchResponseStreamed, setBatchPersist } = store.getState();

      setBatchPersist(sessionId, persist);

      handleBatchResponseStreamed(sessionId, {
        type: "segment",
        percentage: 0.5,
        response: {
          type: "Results",
          start: 0,
          duration: 120,
          is_final: true,
          speech_final: true,
          from_finalize: false,
          channel: {
            alternatives: [
              {
                transcript: "partial",
                languages: [],
                words: [],
                confidence: 0.9,
              },
            ],
          },
          metadata: {
            request_id: "test-request",
            model_info: {
              name: "test-model",
              version: "1.0",
              arch: "test-arch",
            },
            model_uuid: "test-uuid",
          },
          channel_index: [0],
        },
      });

      handleBatchResponseStreamed(sessionId, {
        type: "result",
        response: {
          metadata: { duration: 120, timing_source: "synthetic_text" },
          results: {
            channels: [
              {
                alternatives: [
                  {
                    transcript: "final transcript survived",
                    confidence: 0.9,
                    words: [],
                  },
                ],
              },
            ],
          },
        },
      });

      const finalWords = [
        {
          text: " final",
          start_ms: 0,
          end_ms: 400,
          channel: 0,
          metadata: {
            timing: {
              source: "synthetic_text",
            },
          },
        },
        {
          text: " transcript",
          start_ms: 400,
          end_ms: 800,
          channel: 0,
          metadata: {
            timing: {
              source: "synthetic_text",
            },
          },
        },
        {
          text: " survived",
          start_ms: 800,
          end_ms: 1200,
          channel: 0,
          metadata: {
            timing: {
              source: "synthetic_text",
            },
          },
        },
      ];

      expect(persist).toHaveBeenLastCalledWith(finalWords, [], {
        mode: "replace",
      });
      expect(
        store.getState().batchPreview[sessionId]?.wordsByChannel[0],
      ).toEqual(finalWords);
      expect(store.getState().batch[sessionId]?.isComplete).toBe(true);
    });

    test("handleBatchResponseStreamed persists transcript-only segment responses", () => {
      const sessionId = "session-transcript-only-stream";
      const persist = vi.fn();
      const { handleBatchResponseStreamed, setBatchPersist } = store.getState();

      setBatchPersist(sessionId, persist);

      handleBatchResponseStreamed(sessionId, {
        type: "segment",
        percentage: 0.5,
        response: {
          type: "Results",
          start: 4,
          duration: 2,
          is_final: true,
          speech_final: true,
          from_finalize: false,
          channel: {
            alternatives: [
              {
                transcript: "hello world",
                languages: [],
                words: [],
                confidence: 0.9,
              },
            ],
          },
          metadata: {
            request_id: "test-request",
            model_info: {
              name: "test-model",
              version: "1.0",
              arch: "test-arch",
            },
            model_uuid: "test-uuid",
          },
          channel_index: [1],
        },
      });

      expect(persist).toHaveBeenCalledWith(
        [
          {
            text: " hello",
            start_ms: 4000,
            end_ms: 5000,
            channel: 1,
            metadata: {
              timing: {
                source: "provider_segment_interpolated",
              },
            },
          },
          {
            text: " world",
            start_ms: 5000,
            end_ms: 6000,
            channel: 1,
            metadata: {
              timing: {
                source: "provider_segment_interpolated",
              },
            },
          },
        ],
        [],
        { mode: "replace" },
      );
    });

    test("handleBatchFailed preserves batch error for UI surfaces", () => {
      const sessionId = "session-batch-error";
      const { handleBatchFailed, getSessionMode } = store.getState();

      handleBatchFailed(
        sessionId,
        "batch start failed: connection refused",
        "failed",
      );

      expect(store.getState().batch[sessionId]).toEqual({
        percentage: 0,
        error: "batch start failed: connection refused",
        isComplete: false,
        terminalReason: "failed",
        errorCode: undefined,
      });
      expect(getSessionMode(sessionId)).toBe("inactive");
    });

    test("handleBatchStopped preserves stopped reason for UI surfaces", () => {
      const sessionId = "session-batch-stopped";
      const { handleBatchStopped, getSessionMode } = store.getState();

      handleBatchStopped(sessionId);

      expect(store.getState().batch[sessionId]).toEqual({
        percentage: 0,
        error: "Transcription stopped.",
        isComplete: false,
        terminalReason: "stopped",
        errorCode: undefined,
      });
      expect(getSessionMode(sessionId)).toBe("inactive");
    });
  });

  describe("Stop Action", () => {
    test("stop action exists and is callable", () => {
      const stop = store.getState().stop;
      expect(typeof stop).toBe("function");
    });

    test("marks the live session finalizing immediately when stop is requested", () => {
      const intervalId = setInterval(() => {}, 1000);

      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveActive(draft.live, "session-1", intervalId, true, true, null);
          draft.live.seconds = 42;
        }),
      );

      store.getState().stop();

      expect(store.getState().live.status).toBe("finalizing");
      expect(store.getState().live.loading).toBe(true);
      expect(store.getState().live.intervalId).toBeUndefined();
      expect(store.getState().live.finalizingBySession["session-1"]).toEqual({
        startedAtMs: expect.any(Number),
        seconds: 42,
        needsBatchRepair: false,
      });
    });

    test("dispatches completion after post-stop processing settles", async () => {
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      let finishPostStopProcessing: (() => void) | undefined;
      const intervalId = setInterval(() => {}, 1000);

      listenCaptureLifecycleMock.mockImplementationOnce((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveActive(draft.live, "session-a", intervalId, true, true, null);
        }),
      );
      store.getState().setOnStopped(
        "session-a",
        () =>
          new Promise<void>((resolve) => {
            finishPostStopProcessing = resolve;
          }),
      );

      await store.getState().attachLiveSession("session-a");
      store.getState().stop();
      await vi.waitFor(() => expect(stopCaptureMock).toHaveBeenCalledOnce());
      expect(dispatchEventMock).not.toHaveBeenCalled();

      lifecycleHandler?.({
        payload: {
          type: "stopped",
          session_id: "session-a",
          audio_path: "/tmp/session.wav",
          requested_live_transcription: true,
          live_transcription_active: true,
          error: null,
        },
      });
      expect(dispatchEventMock).not.toHaveBeenCalled();

      finishPostStopProcessing?.();
      await vi.waitFor(() =>
        expect(dispatchEventMock).toHaveBeenCalledWith(
          "meeting.completed",
          "session-a",
        ),
      );
    });
  });

  describe("Start Action", () => {
    test("start action exists and is callable", () => {
      const start = store.getState().start;
      expect(typeof start).toBe("function");
    });

    test("attachLiveSession hydrates the active native capture for the same session", async () => {
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
          liveSegmentsSessionId: "session-a",
          liveSegments: [
            {
              id: "snapshot-segment",
              key: {
                channel: "DirectMic",
                speaker_index: null,
                speaker_human_id: null,
              },
              start_ms: 100,
              end_ms: 200,
              text: "restored",
              words: [],
            },
          ],
        },
      });

      await store.getState().attachLiveSession("session-a");

      expect(store.getState().getSessionMode("session-a")).toBe("active");
      expect(store.getState().live.sessionId).toBe("session-a");
      expect(store.getState().live.liveTranscriptionActive).toBe(true);
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      expect(store.getState().liveSegments).toEqual([
        expect.objectContaining({ id: "snapshot-segment", text: "restored" }),
      ]);
      expect(listenCaptureLifecycleMock).toHaveBeenCalledTimes(1);
      expect(listenCaptureStatusMock).toHaveBeenCalledTimes(1);
      expect(listenCaptureDataMock).toHaveBeenCalledTimes(1);
    });

    test("attached windows advance once for each observed native capture", async () => {
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      listenCaptureLifecycleMock.mockImplementation((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });

      await store.getState().attachLiveSession("session-a");
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBeUndefined();

      const started = {
        type: "started",
        session_id: "session-a",
        requested_live_transcription: true,
        live_transcription_active: true,
        degraded: null,
      };
      const stopped = {
        type: "stopped",
        session_id: "session-a",
        audio_path: "/tmp/session.wav",
        requested_live_transcription: true,
        live_transcription_active: true,
        error: null,
      };

      lifecycleHandler?.({ payload: started });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      lifecycleHandler?.({ payload: started });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      lifecycleHandler?.({
        payload: { type: "finalizing", session_id: "session-a" },
      });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      lifecycleHandler?.({ payload: stopped });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBeUndefined();

      await store.getState().attachLiveSession("session-a");
      lifecycleHandler?.({ payload: started });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(2);
      lifecycleHandler?.({ payload: stopped });
    });

    test("attachLiveSession restores transcript and stop callbacks for an active native capture", async () => {
      let dataHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      const handlePersist = vi.fn();
      const onStopped = vi.fn();
      listenCaptureDataMock.mockImplementationOnce((handler) => {
        dataHandler = handler;
        return Promise.resolve(() => {});
      });
      listenCaptureLifecycleMock.mockImplementationOnce((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });

      await store.getState().attachLiveSession("session-a", {
        handlePersist,
        onStopped,
      });
      dataHandler?.({
        payload: {
          session_id: "session-a",
          type: "transcript_delta",
          delta: {
            new_words: [
              {
                id: "word-after-reload",
                text: " preserved",
                start_ms: 1_000,
                end_ms: 1_500,
                channel: 0,
              },
            ],
            replaced_ids: [],
            partials: [],
          },
        },
      });
      lifecycleHandler?.({
        payload: {
          type: "stopped",
          session_id: "session-a",
          audio_path: "/tmp/session.wav",
          requested_live_transcription: true,
          live_transcription_active: true,
          error: null,
        },
      });

      expect(handlePersist).toHaveBeenCalledWith(
        expect.objectContaining({
          new_words: [expect.objectContaining({ id: "word-after-reload" })],
        }),
      );
      expect(onStopped).toHaveBeenCalledWith(
        "session-a",
        expect.objectContaining({
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
        }),
      );
    });

    test("keeps recovery callbacks installed when the native snapshot is unavailable", async () => {
      const handlePersist = vi.fn();
      const onStopped = vi.fn();
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "error",
        error: "capture snapshot unavailable",
      });

      await expect(
        store.getState().attachLiveSession("session-a", {
          handlePersist,
          onStopped,
        }),
      ).resolves.toBe("error");

      expect(store.getState().handlePersistBySession["session-a"]).toBe(
        handlePersist,
      );
      expect(store.getState().onStoppedBySession["session-a"]).toBe(onStopped);
      expect(
        store.getState().live.eventUnlistenersBySession["session-a"],
      ).toHaveLength(3);
      expect(store.getState().canStartLiveSession("session-a")).toBe(false);
      expect(store.getState().canStartLiveSession("session-b")).toBe(false);
      await expect(
        store.getState().start({
          session_id: "session-b",
          languages: [],
          onboarding: false,
          model: "test-model",
          base_url: "http://localhost",
          api_key: "test-key",
          keywords: [],
        }),
      ).resolves.toBe(false);
      expect(startCaptureMock).not.toHaveBeenCalled();

      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: null,
          finalizingSessionIds: [],
          liveTranscriptionActive: null,
          requestedLiveTranscription: null,
          state: "inactive",
        },
      });

      await expect(
        store.getState().attachLiveSession("session-a", {
          handlePersist: vi.fn(),
          onStopped: vi.fn(),
        }),
      ).resolves.toBe("inactive");
      expect(
        store.getState().handlePersistBySession["session-a"],
      ).toBeUndefined();
      expect(store.getState().onStoppedBySession["session-a"]).toBeUndefined();
      expect(
        store.getState().beginCaptureRecoveryFinalization("session-a"),
      ).toBe(true);
      expect(store.getState().canStartLiveSession("session-a")).toBe(false);
      expect(store.getState().canStartLiveSession("session-b")).toBe(true);
      store.getState().finishCaptureRecoveryFinalization("session-a");
      expect(store.getState().canStartLiveSession("session-a")).toBe(true);
    });

    test("finalizing sessions keep the batch repair flag when another session starts", async () => {
      let lifecycleHandler:
        | ((event: {
            payload:
              | {
                  type: "started";
                  session_id: string;
                  requested_live_transcription: boolean;
                  live_transcription_active: boolean;
                  degraded: { type: "connection_timeout" } | null;
                }
              | {
                  type: "finalizing";
                  session_id: string;
                }
              | {
                  type: "stopped";
                  session_id: string;
                  audio_path: string;
                  requested_live_transcription: boolean;
                  live_transcription_active: boolean;
                  error: null;
                };
          }) => void)
        | undefined;
      const onStopped = vi.fn();
      listenCaptureLifecycleMock.mockImplementationOnce((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      store.getState().setOnStopped("session-a", onStopped);

      await store.getState().attachLiveSession("session-a");
      lifecycleHandler?.({
        payload: {
          type: "started",
          session_id: "session-a",
          requested_live_transcription: true,
          live_transcription_active: false,
          degraded: { type: "connection_timeout" },
        },
      });
      lifecycleHandler?.({
        payload: {
          type: "started",
          session_id: "session-a",
          requested_live_transcription: true,
          live_transcription_active: true,
          degraded: null,
        },
      });
      lifecycleHandler?.({
        payload: {
          type: "finalizing",
          session_id: "session-a",
        },
      });
      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveStartRequested(draft.live, "session-b");
        }),
      );
      lifecycleHandler?.({
        payload: {
          type: "finalizing",
          session_id: "session-a",
        },
      });
      lifecycleHandler?.({
        payload: {
          type: "stopped",
          session_id: "session-a",
          audio_path: "/tmp/session.wav",
          requested_live_transcription: true,
          live_transcription_active: true,
          error: null,
        },
      });

      expect(onStopped).toHaveBeenCalledWith(
        "session-a",
        expect.objectContaining({ needsBatchRepair: true }),
      );
    });

    test("repairs a live transcript when stop leaves unfinalized words", async () => {
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      const onStopped = vi.fn();
      listenCaptureLifecycleMock.mockImplementationOnce((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      store.getState().setOnStopped("session-a", onStopped);

      await store.getState().attachLiveSession("session-a");
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.partialWordsByChannel[0] = [
            {
              text: " trailing words",
              start_ms: 1_000,
              end_ms: 2_000,
              channel: 0,
            },
          ];
        }),
      );

      lifecycleHandler?.({
        payload: {
          type: "finalizing",
          session_id: "session-a",
        },
      });
      lifecycleHandler?.({
        payload: {
          type: "stopped",
          session_id: "session-a",
          audio_path: "/tmp/session.wav",
          requested_live_transcription: true,
          live_transcription_active: true,
          error: null,
        },
      });

      expect(onStopped).toHaveBeenCalledWith(
        "session-a",
        expect.objectContaining({ needsBatchRepair: true }),
      );
    });

    test("observes asynchronous post-stop failures", async () => {
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      const error = new Error("summary scheduling failed");
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      listenCaptureLifecycleMock.mockImplementationOnce((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      store
        .getState()
        .setOnStopped("session-a", vi.fn().mockRejectedValue(error));

      await store.getState().attachLiveSession("session-a");
      lifecycleHandler?.({
        payload: {
          type: "stopped",
          session_id: "session-a",
          audio_path: "/tmp/session.wav",
          requested_live_transcription: true,
          live_transcription_active: true,
          error: null,
        },
      });
      await Promise.resolve();

      expect(consoleError).toHaveBeenCalledWith(
        "[listener] post-stop processing failed",
        error,
      );
      await vi.waitFor(() =>
        expect(store.getState().canStartLiveSession("session-a")).toBe(true),
      );
      consoleError.mockRestore();
    });

    test("blocks an immediate same-session restart until post-stop repair settles", async () => {
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      let finishRepair: (() => void) | undefined;
      const repairCompleted = vi.fn();
      const onStopped = vi.fn(
        (
          _sessionId: string,
          details: { needsBatchRepair: boolean },
        ): Promise<void> => {
          expect(details.needsBatchRepair).toBe(true);
          return new Promise<void>((resolve) => {
            finishRepair = () => {
              repairCompleted();
              resolve();
            };
          });
        },
      );
      listenCaptureLifecycleMock.mockImplementationOnce((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      store.getState().setOnStopped("session-a", onStopped);

      await store.getState().attachLiveSession("session-a");
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.needsBatchRepair = true;
        }),
      );
      lifecycleHandler?.({
        payload: {
          type: "stopped",
          session_id: "session-a",
          audio_path: "/tmp/session.wav",
          requested_live_transcription: true,
          live_transcription_active: true,
          error: null,
        },
      });

      expect(onStopped).toHaveBeenCalledOnce();
      expect(store.getState().canStartLiveSession("session-a")).toBe(false);
      expect(store.getState().canStartLiveSession("session-b")).toBe(true);
      await expect(
        store.getState().start({
          session_id: "session-a",
          languages: [],
          onboarding: false,
          model: "test-model",
          base_url: "http://localhost",
          api_key: "test-key",
          keywords: [],
        }),
      ).resolves.toBe(false);
      expect(startCaptureMock).not.toHaveBeenCalled();
      expect(repairCompleted).not.toHaveBeenCalled();

      finishRepair?.();
      await vi.waitFor(() =>
        expect(store.getState().canStartLiveSession("session-a")).toBe(true),
      );
      expect(repairCompleted).toHaveBeenCalledOnce();
      await expect(
        store.getState().start({
          session_id: "session-a",
          languages: [],
          onboarding: false,
          model: "test-model",
          base_url: "http://localhost",
          api_key: "test-key",
          keywords: [],
        }),
      ).resolves.toBe(true);
      expect(startCaptureMock).toHaveBeenCalledOnce();
    });

    test("attachLiveSession ignores overlapping attaches for the same session", async () => {
      await Promise.all([
        store.getState().attachLiveSession("session-a"),
        store.getState().attachLiveSession("session-a"),
      ]);

      expect(listenCaptureLifecycleMock).toHaveBeenCalledTimes(1);
      expect(listenCaptureStatusMock).toHaveBeenCalledTimes(1);
      expect(listenCaptureDataMock).toHaveBeenCalledTimes(1);
      expect(getCaptureSnapshotMock).toHaveBeenCalledTimes(1);
    });

    test("attachLiveSession clears the previous session interval before applying an active snapshot", async () => {
      const intervalId = setInterval(() => {}, 1000);
      const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-b",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveActive(draft.live, "session-a", intervalId, true, true, null);
        }),
      );

      await store.getState().attachLiveSession("session-b");

      expect(clearIntervalSpy).toHaveBeenCalledWith(intervalId);
      expect(store.getState().live.sessionId).toBe("session-b");

      clearInterval(store.getState().live.intervalId);
      clearIntervalSpy.mockRestore();
    });

    test("attachLiveSession does not mark a different active native capture as current", async () => {
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-b",
          finalizingSessionIds: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });

      await store.getState().attachLiveSession("session-a");

      expect(store.getState().getSessionMode("session-a")).toBe("inactive");
      expect(store.getState().live.sessionId).toBeNull();
    });

    test("attachLiveSession keeps a finalizing target attached while another capture is active", async () => {
      const handlePersist = vi.fn();
      const onStopped = vi.fn();
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-b",
          finalizingSessionIds: ["session-a"],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });

      await expect(
        store.getState().attachLiveSession("session-a", {
          handlePersist,
          onStopped,
        }),
      ).resolves.toBe("attached");

      expect(store.getState().getSessionMode("session-a")).toBe("finalizing");
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      expect(store.getState().handlePersistBySession["session-a"]).toBe(
        handlePersist,
      );
      expect(store.getState().onStoppedBySession["session-a"]).toBe(onStopped);
    });

    test("attachLiveSession does not hydrate another session's snapshot segments", async () => {
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-b",
          finalizingSessionIds: ["session-a"],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
          liveSegmentsSessionId: "session-b",
          liveSegments: [
            {
              id: "session-b-segment",
              key: {
                channel: "DirectMic",
                speaker_index: null,
                speaker_human_id: null,
              },
              start_ms: 100,
              end_ms: 200,
              text: "belongs to session b",
              words: [],
            },
          ],
        },
      });

      await expect(
        store.getState().attachLiveSession("session-a"),
      ).resolves.toBe("attached");

      expect(store.getState().liveSegments).toEqual([]);
    });

    test("attachLiveSession hydrates finalizing native capture for the same session", async () => {
      let dataHandler:
        | ((event: {
            payload: {
              session_id: string;
              type: "transcript_segment_delta";
              delta: unknown;
            };
          }) => void)
        | undefined;
      listenCaptureDataMock.mockImplementationOnce((handler) => {
        dataHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: null,
          finalizingSessionIds: ["session-a"],
          liveTranscriptionActive: null,
          requestedLiveTranscription: null,
          state: "finalizing",
        },
      });

      await store.getState().attachLiveSession("session-a");

      expect(store.getState().live.sessionId).toBe("session-a");
      expect(store.getState().getSessionMode("session-a")).toBe("finalizing");
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);

      dataHandler?.({
        payload: {
          delta: {
            removed_ids: [],
            upserts: [
              {
                end_ms: 1000,
                id: "segment-1",
                key: {
                  channel: "DirectMic",
                },
                start_ms: 0,
                text: "hello",
                words: [],
              },
            ],
          },
          session_id: "session-a",
          type: "transcript_segment_delta",
        },
      });

      expect(store.getState().liveSegments).toMatchObject([
        { id: "segment-1", text: "hello" },
      ]);
    });

    test("attachLiveSession replays segment events after snapshot hydration", async () => {
      let dataHandler:
        | ((event: {
            payload: {
              session_id: string;
              type: "transcript_segment_delta";
              delta: unknown;
            };
          }) => void)
        | undefined;
      let resolveSnapshot:
        | ((value: Awaited<ReturnType<typeof getCaptureSnapshotMock>>) => void)
        | undefined;
      listenCaptureDataMock.mockImplementationOnce((handler) => {
        dataHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSnapshot = resolve;
        }),
      );

      const attachPromise = store.getState().attachLiveSession("session-a");
      await vi.waitFor(() => {
        expect(dataHandler).toBeDefined();
      });

      dataHandler?.({
        payload: {
          delta: {
            removed_ids: [],
            upserts: [
              {
                end_ms: 1000,
                id: "segment-before-snapshot",
                key: {
                  channel: "DirectMic",
                },
                start_ms: 0,
                text: "early",
                words: [],
              },
            ],
          },
          session_id: "session-a",
          type: "transcript_segment_delta",
        },
      });

      expect(store.getState().liveSegments).toEqual([]);

      resolveSnapshot?.({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveSegments: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      await expect(attachPromise).resolves.toBe("attached");
      expect(store.getState().liveSegments).toMatchObject([
        { id: "segment-before-snapshot", text: "early" },
      ]);
    });

    test("attachLiveSession bounds and releases buffered segments when snapshot hydration times out", async () => {
      vi.useFakeTimers();
      try {
        let dataHandler:
          | ((event: {
              payload: {
                session_id: string;
                type: "transcript_segment_delta";
                delta: unknown;
              };
            }) => void)
          | undefined;
        listenCaptureDataMock.mockImplementationOnce((handler) => {
          dataHandler = handler;
          return Promise.resolve(() => {});
        });
        getCaptureSnapshotMock.mockReturnValueOnce(new Promise(() => {}));
        const handleTranscriptSegmentDelta = vi.fn(
          store.getState().handleTranscriptSegmentDelta,
        );
        store.setState({ handleTranscriptSegmentDelta });

        const emitSegmentDelta = (delta: unknown) =>
          dataHandler?.({
            payload: {
              delta,
              session_id: "session-a",
              type: "transcript_segment_delta",
            },
          });
        const attachPromise = store.getState().attachLiveSession("session-a");
        await vi.waitFor(() => {
          expect(dataHandler).toBeDefined();
        });

        for (let index = 0; index < 1_000; index += 1) {
          emitSegmentDelta({
            removed_ids: [],
            upserts: [
              {
                end_ms: index + 1,
                id: `segment-${index}`,
                key: { channel: "DirectMic" },
                start_ms: index,
                text: `segment ${index}`,
                words: [],
              },
            ],
          });
          emitSegmentDelta({
            removed_ids: [`removed-${index}`],
            upserts: [],
          });
        }
        emitSegmentDelta({
          removed_ids: ["segment-999"],
          upserts: [
            {
              end_ms: 999,
              id: "segment-998",
              key: { channel: "DirectMic" },
              start_ms: 998,
              text: "latest segment 998",
              words: [],
            },
          ],
        });

        expect(handleTranscriptSegmentDelta).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(5_000);
        await expect(attachPromise).resolves.toBe("error");

        expect(handleTranscriptSegmentDelta).toHaveBeenCalledOnce();
        const replayedDelta = handleTranscriptSegmentDelta.mock.calls[0]?.[0];
        expect(replayedDelta?.upserts).toHaveLength(199);
        expect(replayedDelta?.removed_ids).toHaveLength(200);
        expect(replayedDelta?.removed_ids).toContain("segment-999");
        expect(replayedDelta?.upserts).toContainEqual(
          expect.objectContaining({
            id: "segment-998",
            text: "latest segment 998",
          }),
        );

        emitSegmentDelta({
          removed_ids: [],
          upserts: [
            {
              end_ms: 2_001,
              id: "segment-after-timeout",
              key: { channel: "DirectMic" },
              start_ms: 2_000,
              text: "still live",
              words: [],
            },
          ],
        });
        expect(handleTranscriptSegmentDelta).toHaveBeenCalledTimes(2);
        expect(store.getState().liveSegments).toContainEqual(
          expect.objectContaining({ id: "segment-after-timeout" }),
        );
      } finally {
        vi.useRealTimers();
      }
    });

    test("an already-subscribed attach does not overwrite newer segment events with its snapshot", async () => {
      let dataHandler:
        | ((event: {
            payload: {
              session_id: string;
              type: "transcript_segment_delta";
              delta: unknown;
            };
          }) => void)
        | undefined;
      let resolveSnapshot:
        | ((value: Awaited<ReturnType<typeof getCaptureSnapshotMock>>) => void)
        | undefined;
      listenCaptureDataMock.mockImplementationOnce((handler) => {
        dataHandler = handler;
        return Promise.resolve(() => {});
      });
      getCaptureSnapshotMock.mockResolvedValueOnce({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveSegments: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      await store.getState().attachLiveSession("session-a");

      getCaptureSnapshotMock.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSnapshot = resolve;
        }),
      );
      const reattachPromise = store.getState().attachLiveSession("session-a", {
        handlePersist: vi.fn(),
      });
      dataHandler?.({
        payload: {
          delta: {
            removed_ids: [],
            upserts: [
              {
                end_ms: 1_000,
                id: "newer-segment",
                key: { channel: "DirectMic" },
                start_ms: 0,
                text: "newer",
                words: [],
              },
            ],
          },
          session_id: "session-a",
          type: "transcript_segment_delta",
        },
      });

      resolveSnapshot?.({
        status: "ok",
        data: {
          activeSessionId: "session-a",
          finalizingSessionIds: [],
          liveSegments: [],
          liveTranscriptionActive: true,
          requestedLiveTranscription: true,
          state: "active",
        },
      });
      await expect(reattachPromise).resolves.toBe("attached");
      expect(store.getState().liveSegments).toMatchObject([
        { id: "newer-segment", text: "newer" },
      ]);

      clearInterval(store.getState().live.intervalId);
    });

    test("start returns false while another session is active", async () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.status = "active";
          draft.live.loading = true;
          draft.live.sessionId = "session-a";
        }),
      );

      const result = await store.getState().start({
        session_id: "session-b",
        languages: [],
        onboarding: false,
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      });

      expect(result).toBe(false);
      expect(store.getState().live.sessionId).toBe("session-a");
    });

    test("holds the session audio lock until capture startup finishes", async () => {
      let resolveStart:
        | ((value: { status: "ok"; data: null }) => void)
        | undefined;
      startCaptureMock.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStart = resolve;
        }),
      );
      const start = store.getState().start({
        session_id: "session-a",
        languages: [],
        onboarding: false,
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      });

      await vi.waitFor(() => expect(startCaptureMock).toHaveBeenCalled());
      const nextOperation = vi.fn(async () => {});
      const queued = enqueueSessionAudioOperation("session-a", nextOperation);
      expect(nextOperation).not.toHaveBeenCalled();

      resolveStart?.({ status: "ok", data: null });
      await expect(start).resolves.toBe(true);
      await queued;
      expect(nextOperation).toHaveBeenCalledOnce();
    });

    test("releases a failed start generation before retrying", async () => {
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      const params = {
        session_id: "session-a",
        languages: [],
        onboarding: false,
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      };
      startCaptureMock.mockResolvedValueOnce({
        status: "error",
        error: "capture unavailable",
      });

      await expect(store.getState().start(params)).resolves.toBe(false);
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBeUndefined();
      expect(store.getState().live.captureGenerationCounter).toBe(1);
      expect(store.getState().live.lastError).toBe("capture unavailable");
      expect(store.getState().live.lastErrorSessionId).toBe("session-a");
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(false);

      await expect(store.getState().start(params)).resolves.toBe(true);
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(2);
      expect(store.getState().live.lastError).toBeNull();
      expect(store.getState().live.lastErrorSessionId).toBeNull();
      consoleError.mockRestore();
    });

    test("keeps a rejected capture startup error visible", async () => {
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      startCaptureMock.mockRejectedValueOnce(
        new Error("audio backend unavailable"),
      );

      await expect(
        store.getState().start({
          session_id: "session-a",
          languages: [],
          onboarding: false,
          model: "test-model",
          base_url: "http://localhost",
          api_key: "test-key",
          keywords: [],
        }),
      ).resolves.toBe(false);

      expect(store.getState().live.lastError).toBe("audio backend unavailable");
      expect(store.getState().live.lastErrorSessionId).toBe("session-a");
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(false);
      consoleError.mockRestore();
    });

    test("does not classify pre-capture startup failures as audio errors", async () => {
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      vaultBaseMock.mockResolvedValueOnce({
        status: "error",
        error: "storage unavailable",
      });

      await expect(
        store.getState().start({
          session_id: "session-a",
          languages: [],
          onboarding: false,
          model: "test-model",
          base_url: "http://localhost",
          api_key: "test-key",
          keywords: [],
        }),
      ).resolves.toBe(false);

      expect(startCaptureMock).not.toHaveBeenCalled();
      expect(store.getState().live.lastError).toBe("storage unavailable");
      expect(store.getState().live.lastErrorSessionId).toBe("session-a");
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(false);
      consoleError.mockRestore();
    });

    test("preserves explicit audio errors when capture startup fails", async () => {
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      let resolveStart:
        | ((value: { status: "error"; error: string }) => void)
        | undefined;
      startCaptureMock.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStart = resolve;
        }),
      );

      const start = store.getState().start({
        session_id: "session-a",
        languages: [],
        onboarding: false,
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      });

      await vi.waitFor(() => expect(startCaptureMock).toHaveBeenCalled());
      const progressHandler =
        listenCaptureStatusMock.mock.calls[
          listenCaptureStatusMock.mock.calls.length - 1
        ]?.[0];
      progressHandler?.({
        payload: {
          type: "audio_error",
          session_id: "session-a",
          error: "microphone unavailable",
          is_fatal: true,
        },
      });
      resolveStart?.({ status: "error", error: "start session failed" });

      await expect(start).resolves.toBe(false);
      expect(store.getState().live.lastError).toBe("microphone unavailable");
      expect(store.getState().live.lastErrorSessionId).toBe("session-a");
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(true);
      consoleError.mockRestore();
    });

    test("does not classify connection errors as audio startup failures", async () => {
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      let resolveStart:
        | ((value: { status: "error"; error: string }) => void)
        | undefined;
      startCaptureMock.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStart = resolve;
        }),
      );

      const start = store.getState().start({
        session_id: "session-a",
        languages: [],
        onboarding: false,
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      });

      await vi.waitFor(() => expect(startCaptureMock).toHaveBeenCalled());
      const progressHandler =
        listenCaptureStatusMock.mock.calls[
          listenCaptureStatusMock.mock.calls.length - 1
        ]?.[0];
      progressHandler?.({
        payload: {
          type: "connection_error",
          session_id: "session-a",
          error: "transcription connection closed",
        },
      });
      resolveStart?.({ status: "error", error: "start session failed" });

      await expect(start).resolves.toBe(false);
      expect(store.getState().live.lastError).toBe("start session failed");
      expect(store.getState().live.lastErrorSessionId).toBe("session-a");
      expect(store.getState().live.lastErrorIsAudioRelated).toBe(false);
      consoleError.mockRestore();
    });

    test("keeps the capture generation when stop command delivery fails", async () => {
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      const intervalId = setInterval(() => {}, 1000);
      store.setState((state) =>
        mutate(state, (draft) => {
          markLiveActive(draft.live, "session-a", intervalId, true, true, null);
        }),
      );
      stopCaptureMock.mockResolvedValueOnce({
        status: "error",
        error: "stop unavailable",
      });

      store.getState().stop();

      await vi.waitFor(() =>
        expect(store.getState().live.status).toBe("active"),
      );
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      clearInterval(store.getState().live.intervalId);
      consoleError.mockRestore();
    });

    test("deduplicates capture generation across native start event ordering", async () => {
      let lifecycleHandler:
        | ((event: { payload: Record<string, unknown> }) => void)
        | undefined;
      let resolveStart:
        | ((value: { status: "ok"; data: null }) => void)
        | undefined;
      listenCaptureLifecycleMock.mockImplementation((handler) => {
        lifecycleHandler = handler;
        return Promise.resolve(() => {});
      });
      startCaptureMock.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStart = resolve;
        }),
      );
      const params = {
        session_id: "session-a",
        languages: [],
        onboarding: false,
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      };
      const started = {
        type: "started",
        session_id: "session-a",
        requested_live_transcription: true,
        live_transcription_active: true,
        degraded: null,
      };
      const stopped = {
        type: "stopped",
        session_id: "session-a",
        audio_path: "/tmp/session.wav",
        requested_live_transcription: true,
        live_transcription_active: true,
        error: null,
      };

      const firstStart = store.getState().start(params);
      await vi.waitFor(() => expect(startCaptureMock).toHaveBeenCalledOnce());
      lifecycleHandler?.({ payload: started });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      resolveStart?.({ status: "ok", data: null });
      await expect(firstStart).resolves.toBe(true);
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(1);
      lifecycleHandler?.({ payload: stopped });

      await expect(store.getState().start(params)).resolves.toBe(true);
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(2);
      lifecycleHandler?.({ payload: started });
      expect(
        store.getState().live.captureGenerationBySession["session-a"],
      ).toBe(2);
      lifecycleHandler?.({ payload: stopped });
    });

    test("getSessionMode returns finalizing for non-active finalizing sessions", () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.finalizingBySession["session-a"] = {
            startedAtMs: 123,
            seconds: 0,
            needsBatchRepair: false,
          };
        }),
      );

      expect(store.getState().getSessionMode("session-a")).toBe("finalizing");
    });

    test("canStartLiveSession allows new sessions while another session is finalizing", () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.status = "finalizing";
          draft.live.loading = true;
          draft.live.sessionId = "session-a";
          draft.live.finalizingBySession["session-a"] = {
            startedAtMs: 123,
            seconds: 0,
            needsBatchRepair: false,
          };
        }),
      );

      expect(store.getState().canStartLiveSession("session-b")).toBe(true);
      expect(store.getState().canStartLiveSession("session-a")).toBe(false);
    });

    test("canStartLiveSession blocks new sessions while another session is active", () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.status = "active";
          draft.live.sessionId = "session-a";
        }),
      );

      expect(store.getState().canStartLiveSession("session-b")).toBe(false);
    });

    test("startTranscription rejects when the session is already running batch", async () => {
      const sessionId = "session-batch";
      store.getState().handleBatchStarted(sessionId);

      await expect(
        store.getState().startTranscription({
          session_id: sessionId,
          provider: "anarlog",
          file_path: "/tmp/session.wav",
          base_url: "",
          api_key: "",
        }),
      ).rejects.toThrow(
        `[listener] session ${sessionId} is already processing in batch mode`,
      );
    });
  });
});
