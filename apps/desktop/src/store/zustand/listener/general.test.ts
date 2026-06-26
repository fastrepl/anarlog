import { create as mutate } from "mutative";
import { beforeEach, describe, expect, test, vi } from "vitest";

const {
  getIdentifierMock,
  runEventHooksMock,
  setRecordingIndicatorMock,
  stopCaptureMock,
  vaultBaseMock,
} = vi.hoisted(() => ({
  getIdentifierMock: vi.fn(),
  runEventHooksMock: vi.fn(),
  setRecordingIndicatorMock: vi.fn(),
  stopCaptureMock: vi.fn(),
  vaultBaseMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: getIdentifierMock,
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: vi.fn(),
  },
}));

vi.mock("@hypr/plugin-hooks", () => ({
  commands: {
    runEventHooks: runEventHooksMock,
  },
}));

vi.mock("@hypr/plugin-icon", () => ({
  commands: {
    setRecordingIndicator: setRecordingIndicatorMock,
  },
}));

vi.mock("@hypr/plugin-settings", () => ({
  commands: {
    vaultBase: vaultBaseMock,
  },
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: {
    setMicMuted: vi.fn(),
    startCapture: vi.fn(),
    startTranscription: vi.fn(),
    stopCapture: stopCaptureMock,
    stopTranscription: vi.fn(),
    updateCaptureConfig: vi.fn(),
  },
  events: {
    captureDataEvent: {
      listen: vi.fn(),
    },
    captureLifecycleEvent: {
      listen: vi.fn(),
    },
    captureStatusEvent: {
      listen: vi.fn(),
    },
  },
}));

import { createListenerStore } from ".";
import {
  getLiveCaptureUiMode,
  markLiveActive,
  updateLiveProgress,
} from "./general-shared";

let store: ReturnType<typeof createListenerStore>;

describe("General Listener Slice", () => {
  beforeEach(() => {
    store = createListenerStore();
    vi.clearAllMocks();
    getIdentifierMock.mockResolvedValue("com.hyprnote.stable");
    runEventHooksMock.mockResolvedValue({ status: "ok", data: null });
    setRecordingIndicatorMock.mockResolvedValue({ status: "ok", data: null });
    stopCaptureMock.mockResolvedValue({ status: "ok", data: null });
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
      });
    });
  });

  describe("Start Action", () => {
    test("start action exists and is callable", () => {
      const start = store.getState().start;
      expect(typeof start).toBe("function");
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

    test("getSessionMode returns finalizing for non-active finalizing sessions", () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.finalizingBySession["session-a"] = {
            startedAtMs: 123,
            seconds: 0,
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
          provider: "hyprnote",
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
