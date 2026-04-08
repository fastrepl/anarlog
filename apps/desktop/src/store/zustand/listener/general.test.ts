import { create as mutate } from "mutative";
import { beforeEach, describe, expect, test } from "vitest";

import { createListenerStore } from ".";
import { getLiveCaptureUiMode } from "./general-shared";

let store: ReturnType<typeof createListenerStore>;

describe("General Listener Slice", () => {
  beforeEach(() => {
    store = createListenerStore();
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
        },
      ]);

      clearBatchSession(sessionId);
      expect(store.getState().batch[sessionId]).toBeUndefined();
      expect(store.getState().batchPreview[sessionId]).toBeUndefined();
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
          draft.live.finalizingBySession["session-a"] = { startedAtMs: 123 };
        }),
      );

      expect(store.getState().getSessionMode("session-a")).toBe("finalizing");
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
